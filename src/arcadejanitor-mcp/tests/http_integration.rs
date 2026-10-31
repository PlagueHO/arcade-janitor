use std::{
    fs::{self, File},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use reqwest::blocking::Client;
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ProtocolVersion},
    service::{ClientLifecycleMode, ClientServiceExt},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest};

#[path = "../../../tests/support/mod.rs"]
mod support;

use support::{FlowFixture, PACMAN_BYTES};

struct ServerProcess {
    child: Child,
    diagnostics: PathBuf,
    _directory: tempfile::TempDir,
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(error) => eprintln!("cannot inspect MCP test server during cleanup: {error}"),
        }
        if let Err(error) = self.child.kill() {
            eprintln!("cannot stop MCP test server: {error}");
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    eprintln!("cannot reap MCP test server: {error}");
                    return;
                }
            }
        }
        eprintln!("MCP test server cleanup exceeded 5 seconds");
    }
}

impl ServerProcess {
    fn stderr(&self) -> String {
        fs::read_to_string(&self.diagnostics).expect("read captured MCP diagnostics")
    }
}

struct StdioProcess(tokio::process::Child);

impl StdioProcess {
    fn stop_and_reap(&mut self) {
        match self.0.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(error) => eprintln!("cannot inspect stdio MCP test server during cleanup: {error}"),
        }
        if let Err(error) = self.0.start_kill() {
            eprintln!("cannot stop stdio MCP test server: {error}");
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match self.0.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    eprintln!("cannot reap stdio MCP test server: {error}");
                    return;
                }
            }
        }
        eprintln!("stdio MCP test server cleanup exceeded 5 seconds");
    }
}

impl Drop for StdioProcess {
    fn drop(&mut self) {
        self.stop_and_reap();
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn start_server(port: u16, rom_folder: &Path, mame_xml: &Path, catver: &Path) -> ServerProcess {
    start_server_with_token(port, rom_folder, mame_xml, catver, None)
}

fn start_server_with_token(
    port: u16,
    rom_folder: &Path,
    mame_xml: &Path,
    catver: &Path,
    token: Option<&str>,
) -> ServerProcess {
    let directory = tempfile::tempdir().unwrap();
    let diagnostics = directory.path().join("stderr.txt");
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcadejanitor-mcp"));
    for name in support::inherited_application_settings() {
        command.env_remove(name);
    }
    command
        .args([
            "--rom-folder",
            rom_folder.to_str().unwrap(),
            "--mame-xml",
            mame_xml.to_str().unwrap(),
            "--catver",
            catver.to_str().unwrap(),
        ])
        .envs(support::environment(directory.path()))
        .current_dir(directory.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(File::create(&diagnostics).unwrap())
        .env("ARCADEJANITOR_MCP_ADDR", format!("127.0.0.1:{port}"))
        .env("RUST_LOG", "arcadejanitor_mcp=info");
    if let Some(token) = token {
        command.env("ARCADEJANITOR_MCP_TOKEN", token);
    }
    let child = command.spawn().unwrap();
    ServerProcess {
        child,
        diagnostics,
        _directory: directory,
    }
}

fn readiness(
    server: &mut ServerProcess,
    client: &Client,
    url: &str,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    let mut last_response = String::from("no response");
    while Instant::now() < deadline {
        if let Some(status) = server.child.try_wait().map_err(|error| error.to_string())? {
            return Err(format!(
                "MCP command {} exited before readiness: {status}\nstderr: {}",
                env!("CARGO_BIN_EXE_arcadejanitor-mcp"),
                server.stderr()
            ));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match client
            .get(format!("{url}/health"))
            .timeout(remaining.min(Duration::from_millis(200)))
            .send()
        {
            Ok(response) => {
                let status = response.status();
                let body = response.text().map_err(|error| error.to_string())?;
                if status.is_success() && body == "ok (arcadejanitor)" {
                    return Ok(());
                }
                last_response = format!("HTTP {status}, body: {body:?}");
            }
            Err(error) => last_response = error.to_string(),
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(format!(
        "MCP command {} did not become ready at {url}: {last_response}\nstderr: {}",
        env!("CARGO_BIN_EXE_arcadejanitor-mcp"),
        server.stderr()
    ))
}

fn wait_for_server(server: &mut ServerProcess, client: &Client, url: &str) {
    readiness(server, client, url, Duration::from_secs(10))
        .unwrap_or_else(|error| panic!("{error}"));
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[derive(Debug)]
struct HttpExchange {
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
    request: Value,
    status: axum::http::StatusCode,
    response_headers: axum::http::HeaderMap,
    response: String,
}

struct HttpObserver {
    url: String,
    exchanges: Arc<Mutex<Vec<HttpExchange>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for HttpObserver {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl HttpObserver {
    async fn start(upstream: &str) -> Self {
        use axum::{
            Router,
            body::Bytes,
            http::{HeaderMap, Method},
            routing::any,
        };

        let exchanges = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&exchanges);
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let upstream = format!("{upstream}/mcp");
        let app = Router::new().route(
            "/mcp",
            any(move |method: Method, headers: HeaderMap, body: Bytes| {
                let client = client.clone();
                let upstream = upstream.clone();
                let recorded = Arc::clone(&recorded);
                async move {
                    // Only Host changes for the upstream address; MCP bytes and headers do not.
                    let mut forwarded_headers = headers.clone();
                    forwarded_headers.remove(axum::http::header::HOST);
                    let response = client
                        .request(method.clone(), upstream)
                        .headers(forwarded_headers)
                        .body(body.to_vec())
                        .send()
                        .await
                        .expect("forward SDK HTTP request");
                    let status = response.status();
                    let response_headers = response.headers().clone();
                    let response = response.bytes().await.expect("read upstream response");
                    recorded.lock().unwrap().push(HttpExchange {
                        method,
                        headers,
                        request: if body.is_empty() {
                            Value::Null
                        } else {
                            serde_json::from_slice(&body).unwrap()
                        },
                        status,
                        response_headers: response_headers.clone(),
                        response: String::from_utf8(response.to_vec()).unwrap(),
                    });
                    (status, response_headers, response)
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve HTTP observer");
        });
        Self {
            url,
            exchanges,
            task,
        }
    }
}

fn sdk_http_transport(
    url: &str,
    token: Option<&str>,
) -> StreamableHttpClientTransport<sdk_reqwest::Client> {
    let client = sdk_reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let mut config = StreamableHttpClientTransportConfig::with_uri(url.to_owned());
    if let Some(token) = token {
        config = config.auth_header(token);
    }
    StreamableHttpClientTransport::with_client(client, config)
}

#[test]
fn sdk_http_client_reads_populated_custom_transport() {
    let fixture = FlowFixture::new();
    let port = free_port();
    let mut server = start_server_with_token(
        port,
        &fixture.roms,
        &fixture.mame_xml,
        &fixture.catver,
        Some("test-token"),
    );
    let url = format!("http://127.0.0.1:{port}");
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    wait_for_server(&mut server, &client, &url);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(60), async {
            for token in [None, Some("wrong"), Some("test-token")] {
                let observer = HttpObserver::start(&url).await;
                let transport = sdk_http_transport(&observer.url, token);
                let mut sdk = tokio::time::timeout(Duration::from_secs(5), ().serve(transport))
                    .await
                    .expect("SDK HTTP initialization timed out")
                    .unwrap_or_else(|error| {
                        panic!(
                            "SDK HTTP initialization failed: {error}\nstderr: {}",
                            server.stderr()
                        )
                    });
                assert_eq!(
                    sdk.peer_info().unwrap().protocol_version,
                    ProtocolVersion::V_2024_11_05
                );
                let tools = tokio::time::timeout(Duration::from_secs(5), sdk.list_tools(None))
                    .await
                    .expect("SDK HTTP tool discovery timed out")
                    .unwrap();
                let authorized = token == Some("test-token");
                assert_eq!(tools.tools.len(), if authorized { 9 } else { 7 });
                assert!(tools.tools.iter().any(|tool| tool.name == "scan_roms"));
                for name in ["move_roms", "delete_roms"] {
                    assert_eq!(tools.tools.iter().any(|tool| tool.name == name), authorized);
                }
                let scan = sdk_call(&sdk, "scan_roms", json!({}), &server.diagnostics).await;
                assert_ne!(scan.is_error, Some(true));
                support::assert_initial_scan(&sdk_payload(&scan)["roms"], true);
                if !authorized {
                    let denied = sdk_call(
                        &sdk,
                        "delete_roms",
                        json!({"filter": {}, "dry_run": false}),
                        &server.diagnostics,
                    )
                    .await;
                    assert_eq!(denied.is_error, Some(true));
                    assert!(
                        denied.content[0]
                            .as_text()
                            .unwrap()
                            .text
                            .contains("authorization is required")
                    );
                }
                tokio::time::timeout(Duration::from_secs(5), sdk.close())
                    .await
                    .expect("SDK HTTP close timed out")
                    .unwrap();

                let exchanges = observer.exchanges.lock().unwrap();
                let methods: Vec<_> = exchanges
                    .iter()
                    .map(|exchange| exchange.request["method"].as_str().unwrap())
                    .collect();
                assert_eq!(
                    &methods[..4],
                    [
                        "initialize",
                        "notifications/initialized",
                        "tools/list",
                        "tools/call"
                    ]
                );
                assert_eq!(methods.len(), if authorized { 4 } else { 5 });
                assert_eq!(
                    exchanges[0].request["params"]["protocolVersion"],
                    "2026-07-28"
                );
                let initialized: Value = serde_json::from_str(&exchanges[0].response).unwrap();
                assert_eq!(initialized["result"]["protocolVersion"], "2024-11-05");
                for (index, exchange) in exchanges.iter().enumerate() {
                    assert_eq!(exchange.method, axum::http::Method::POST);
                    let accept = exchange.headers["accept"].to_str().unwrap();
                    assert!(accept.contains("application/json"));
                    assert!(accept.contains("text/event-stream"));
                    assert_eq!(exchange.headers["content-type"], "application/json");
                    assert_eq!(
                        exchange.headers.get("authorization"),
                        token
                            .map(|token| format!("Bearer {token}").parse().unwrap())
                            .as_ref()
                    );
                    assert!(exchange.headers.get("mcp-session-id").is_none());
                    assert!(exchange.response_headers.get("mcp-session-id").is_none());
                    assert!(exchange.response_headers.get("www-authenticate").is_none());
                    if index == 0 {
                        assert!(exchange.headers.get("mcp-protocol-version").is_none());
                    } else {
                        assert_eq!(exchange.headers["mcp-protocol-version"], "2024-11-05");
                    }
                    if index == 1 {
                        assert_eq!(exchange.status, 202);
                        assert!(exchange.response.is_empty());
                    } else {
                        assert_eq!(exchange.status, 200);
                        assert_eq!(
                            exchange.response_headers["content-type"],
                            "application/json"
                        );
                        let response: Value = serde_json::from_str(&exchange.response).unwrap();
                        assert_eq!(response["id"], exchange.request["id"]);
                    }
                }
            }
        })
        .await
        .expect("SDK HTTP evaluation exceeded 60 seconds");
    });
    assert_eq!(
        fs::read(fixture.roms.join("pacman.zip")).unwrap(),
        PACMAN_BYTES
    );
    fixture.assert_unrelated_files();
    drop(server);
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
}

#[test]
#[ignore = "modern HTTP discovery is unsupported; run explicitly to reproduce the compatibility gap"]
fn sdk_modern_http_discovery_compatibility() {
    let fixture = FlowFixture::new();
    let port = free_port();
    let mut server = start_server(port, &fixture.roms, &fixture.mame_xml, &fixture.catver);
    let url = format!("http://127.0.0.1:{port}");
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    wait_for_server(&mut server, &client, &url);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let observer = HttpObserver::start(&url).await;
        let transport = sdk_http_transport(&observer.url, None);
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            ().serve_with_lifecycle(
                transport,
                ClientLifecycleMode::Discover {
                    preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                },
            ),
        )
        .await
        .expect("modern SDK HTTP discovery timed out");
        let mut sdk = result.unwrap_or_else(|error| {
            panic!(
                "modern SDK HTTP discovery failed: {error}\nexchanges: {:#?}\nstderr: {}",
                observer.exchanges.lock().unwrap(),
                server.stderr()
            )
        });
        tokio::time::timeout(Duration::from_secs(5), sdk.close())
            .await
            .expect("modern SDK HTTP close timed out")
            .unwrap();
    });
}

#[test]
fn records_custom_http_contract_gaps() {
    let fixture = FlowFixture::new();
    let port = free_port();
    let mut server = start_server(port, &fixture.roms, &fixture.mame_xml, &fixture.catver);
    let url = format!("http://127.0.0.1:{port}");
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    wait_for_server(&mut server, &client, &url);
    for method in [reqwest::Method::GET, reqwest::Method::DELETE] {
        let response = client
            .request(method.clone(), format!("{url}/mcp"))
            .header("Accept", "application/json, text/event-stream")
            .send()
            .unwrap();
        assert_eq!(response.status(), 405, "{method}");
        assert!(response.headers().get("mcp-session-id").is_none());
    }
    for (version, method, name, origin) in [
        ("2026-07-28", "tools/call", "scan_roms", "http://localhost"),
        ("2024-11-05", "tools/call", "scan_roms", "http://localhost"),
        ("invalid", "tools/call", "scan_roms", "http://localhost"),
        ("2026-07-28", "ping", "scan_roms", "http://localhost"),
        (
            "2026-07-28",
            "tools/call",
            "not-scan-roms",
            "http://localhost",
        ),
        (
            "2026-07-28",
            "tools/call",
            "scan_roms",
            "https://example.com",
        ),
    ] {
        let response = client
            .post(format!("{url}/mcp"))
            .header("MCP-Protocol-Version", version)
            .header("Origin", origin)
            .header("Accept", "application/json, text/event-stream")
            .header("Mcp-Method", method)
            .header("Mcp-Name", name)
            .json(&json!({
                "jsonrpc": "2.0", "id": "metadata", "method": "tools/call",
                "params": {
                    "name": "scan_roms", "arguments": {},
                    "_meta": {
                        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                        "io.modelcontextprotocol/clientInfo": {"name": "evaluation", "version": "1"},
                        "io.modelcontextprotocol/clientCapabilities": {}
                    }
                }
            }))
            .send()
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(response.headers().get("www-authenticate").is_none());
        let response: Value = response.json().unwrap();
        let result: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        support::assert_initial_scan(&result["roms"], true);
    }
    for path in [
        "/.well-known/oauth-protected-resource",
        "/.well-known/oauth-protected-resource/mcp",
    ] {
        assert_eq!(
            client.get(format!("{url}{path}")).send().unwrap().status(),
            404
        );
    }
    assert_eq!(
        fs::read(fixture.roms.join("pacman.zip")).unwrap(),
        PACMAN_BYTES
    );
    fixture.assert_unrelated_files();
}

fn call_tool(client: &Client, url: &str, name: &str, arguments: Value) -> Value {
    http_call(client, url, name, arguments, None)
}

fn http_call(
    client: &Client,
    url: &str,
    name: &str,
    arguments: Value,
    token: Option<&str>,
) -> Value {
    let mut request = client.post(format!("{url}/mcp")).json(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    }));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    request
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .unwrap()
}

#[test]
fn serves_mcp_tools_over_http_using_committed_fixture_and_temp_roms() {
    let directory = tempfile::tempdir().unwrap();
    let roms = directory.path().join("roms");
    std::fs::create_dir(&roms).unwrap();
    for name in ["game001.zip", "game050.7z", "unmatched.zip"] {
        std::fs::write(roms.join(name), b"fixture rom").unwrap();
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mame_xml = fixture("mame-100.xml");
    let catver = fixture("catver-100.ini");
    let mut server = start_server(port, &roms, &mame_xml, &catver);
    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .no_proxy()
        .build()
        .unwrap();
    let url = format!("http://127.0.0.1:{port}");
    wait_for_server(&mut server, &client, &url);

    for arguments in [None, Some(json!({}))] {
        let mut params = json!({ "name": "scan_roms" });
        if let Some(arguments) = arguments {
            params["arguments"] = arguments;
        }
        let response: Value = client
            .post(format!("{url}/mcp"))
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": params
            }))
            .send()
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(response["result"]["isError"], Value::Null);
        let content = response["result"]["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(content).unwrap();
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["roms"].as_array().unwrap().len(), 101);
        assert_eq!(payload["roms"][0]["name"], "game001");
        let available: Vec<_> = payload["roms"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| !row["rom_path"].is_null())
            .map(|row| row["name"].as_str().unwrap())
            .collect();
        assert_eq!(available, ["game001", "game050", "unmatched"]);
        assert_eq!(payload["roms"][100]["catalogued"], false);
    }

    for arguments in [Value::Null, json!([1]), json!("invalid")] {
        let response = call_tool(&client, &url, "scan_roms", arguments);
        assert_eq!(response["result"]["isError"], true);
    }
}

#[test]
fn preserves_stateless_http_protocol_and_authorization() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("game001.zip"), b"protected archive").unwrap();
    std::fs::write(directory.path().join("notes.txt"), b"protected note").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut server = start_server_with_token(
        port,
        directory.path(),
        &fixture("mame-100.xml"),
        &fixture("catver-100.ini"),
        Some("test-token"),
    );
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .build()
        .unwrap();
    let url = format!("http://127.0.0.1:{port}");
    wait_for_server(&mut server, &client, &url);
    let request = |body: Value, token: Option<&str>| {
        let mut request = client.post(format!("{url}/mcp")).json(&body);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request.send().unwrap()
    };

    let list_request = json!({"jsonrpc": "2.0", "id": "tools", "method": "tools/list"});
    for token in [None, Some("wrong")] {
        let response: Value = request(list_request.clone(), token).json().unwrap();
        assert_eq!(response["id"], "tools");
        assert_eq!(response["jsonrpc"], "2.0");
        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 7);
        assert!(
            tools
                .iter()
                .all(|tool| { tool["name"] != "move_roms" && tool["name"] != "delete_roms" })
        );
        for name in ["move_roms", "delete_roms"] {
            let mut arguments =
                json!({"filter": {"options": {"names": ["game001"]}}, "dry_run": false});
            if name == "move_roms" {
                arguments["target_folder"] = json!(directory.path().join("moved"));
            }
            let response = http_call(&client, &url, name, arguments, token);
            assert_eq!(response["result"]["isError"], true);
            assert!(
                response["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains("authorization is required")
            );
            assert_eq!(
                fs::read(directory.path().join("game001.zip")).unwrap(),
                b"protected archive"
            );
            assert_eq!(
                fs::read(directory.path().join("notes.txt")).unwrap(),
                b"protected note"
            );
            assert!(!directory.path().join("moved").exists());
        }
    }
    let response: Value = request(list_request, Some("test-token")).json().unwrap();
    let names: Vec<_> = response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "scan_roms",
            "query_metadata",
            "filter_roms",
            "show_mame_xml",
            "filter_mame_metadata",
            "move_roms",
            "delete_roms",
            "generate_report",
            "list_catver",
        ]
    );
    for tool in response["result"]["tools"].as_array().unwrap() {
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(
            !tool["inputSchema"]["properties"]
                .as_object()
                .unwrap()
                .contains_key("rom_folder")
        );
        if tool["name"] == "move_roms" || tool["name"] == "delete_roms" {
            assert_eq!(
                tool["inputSchema"]["properties"]["dry_run"]["default"],
                true
            );
        }
    }

    for (id, method) in [(json!(7), "ping"), (json!("unknown"), "unknown/method")] {
        let response: Value = request(json!({"jsonrpc": "2.0", "id": id, "method": method}), None)
            .json()
            .unwrap();
        assert_eq!(response["id"], id);
        if method == "ping" {
            assert_eq!(response["result"], json!({}));
        } else {
            assert_eq!(response["error"]["code"], -32601);
        }
    }
    let response: Value = request(
        json!({"jsonrpc": "2.0", "id": 8, "method": "initialize"}),
        None,
    )
    .json()
    .unwrap();
    assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(response["result"]["serverInfo"]["name"], "arcadejanitor");
    assert_eq!(
        response["result"]["capabilities"]["tools"]["listChanged"],
        false
    );

    let response = request(
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        None,
    );
    assert_eq!(response.status(), 202);
    assert!(response.text().unwrap().is_empty());

    for body in [
        json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call"}),
        json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {}}),
    ] {
        let response: Value = request(body, None).json().unwrap();
        assert_eq!(response["error"]["code"], -32602);
    }
    for (name, arguments, token) in [
        ("delete_roms", json!({"filter": {}, "dry_run": true}), None),
        ("unknown", json!({}), None),
        ("query_metadata", json!({}), None),
        ("scan_roms", json!({"rom_folder": "agent-path"}), None),
    ] {
        let response: Value = request(
            json!({
                "jsonrpc": "2.0", "id": "failure", "method": "tools/call",
                "params": {"name": name, "arguments": arguments}
            }),
            token,
        )
        .json()
        .unwrap();
        assert_eq!(response["result"]["isError"], true, "{response}");
        assert!(response["error"].is_null(), "{response}");
        assert!(
            !response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .is_empty()
        );
    }
    let response: Value = request(json!({"invalid": true}), None).json().unwrap();
    assert_eq!(response["error"]["code"], -32600);

    let response: Value = request(
        json!({
            "jsonrpc": "2.0", "id": 10, "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "compatibility-test", "version": "1"}
            }
        }),
        None,
    )
    .json()
    .unwrap();
    assert_eq!(response["result"]["protocolVersion"], "2024-11-05");

    let ws_url = format!("ws://127.0.0.1:{port}/ws");
    let mut untrusted = ws_url.as_str().into_client_request().unwrap();
    untrusted
        .headers_mut()
        .insert("Origin", "https://example.com".parse().unwrap());
    match tungstenite::client(untrusted, websocket_stream(port)).unwrap_err() {
        tungstenite::HandshakeError::Failure(tungstenite::Error::Http(response)) => {
            assert_eq!(response.status(), 403)
        }
        error => panic!("unexpected WebSocket rejection: {error}"),
    }
    let mut trusted = ws_url.as_str().into_client_request().unwrap();
    trusted
        .headers_mut()
        .insert("Origin", "http://localhost:3000".parse().unwrap());
    trusted
        .headers_mut()
        .insert("Authorization", "Bearer test-token".parse().unwrap());
    let (mut socket, _) = tungstenite::client(trusted, websocket_stream(port)).unwrap();
    socket.send(tungstenite::Message::text("not JSON")).unwrap();
    let response: Value = serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(response["error"]["code"], -32700);
    socket
        .send(tungstenite::Message::text(
            json!({"jsonrpc": "2.0", "id": "websocket-tools", "method": "tools/list"}).to_string(),
        ))
        .unwrap();
    let response: Value = serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(response["id"], "websocket-tools");
    assert_eq!(response["result"]["tools"].as_array().unwrap().len(), 9);
    socket.close(None).unwrap();
}

fn websocket_stream(port: u16) -> TcpStream {
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
}

#[tokio::test]
async fn serves_stdio_to_the_official_sdk_client() {
    tokio::time::timeout(Duration::from_secs(60), async {
        for token in [None, Some("test-token")] {
            let fixture = FlowFixture::new();
            let diagnostics = fixture.directory.path().join("stderr.txt");
            let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_arcadejanitor-mcp"));
            for name in support::inherited_application_settings() {
                command.env_remove(name);
            }
            command
                .arg("--transport")
                .arg("stdio")
                .arg("--rom-folder")
                .arg(&fixture.roms)
                .envs(support::environment(fixture.directory.path()))
                .current_dir(fixture.directory.path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(File::create(&diagnostics).unwrap())
                .env("RUST_LOG", "arcadejanitor_mcp=info")
                .kill_on_drop(true);
            match token {
                Some(token) => {
                    command.env("ARCADEJANITOR_MCP_TOKEN", token);
                    let cache = fixture.directory.path().join("cache");
                    fs::write(cache.join("mame.xml"), support::MAME_XML).unwrap();
                    fs::write(cache.join("catver.ini"), support::CATVER).unwrap();
                }
                None => {
                    command.env_remove("ARCADEJANITOR_MCP_TOKEN");
                    command
                        .arg("--mame-xml")
                        .arg(&fixture.mame_xml)
                        .arg("--catver")
                        .arg(&fixture.catver);
                }
            }
            let mut child = StdioProcess(command.spawn().unwrap());
            let transport = (
                child.0.stdout.take().unwrap(),
                child.0.stdin.take().unwrap(),
            );
            let mut client =
                match tokio::time::timeout(Duration::from_secs(10), ().serve(transport)).await {
                    Ok(Ok(client)) => client,
                    Ok(Err(error)) => panic!(
                        "stdio initialization failed: {error}\nstderr: {}",
                        fs::read_to_string(&diagnostics).unwrap()
                    ),
                    Err(error) => panic!(
                        "stdio initialization timed out: {error}\nstderr: {}",
                        fs::read_to_string(&diagnostics).unwrap()
                    ),
                };
            let tools = tokio::time::timeout(Duration::from_secs(5), client.list_tools(None))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(tools.tools.len(), if token.is_some() { 9 } else { 7 });
            let omitted = tokio::time::timeout(
                Duration::from_secs(5),
                client.call_tool(CallToolRequestParams::new("scan_roms")),
            )
            .await
            .unwrap()
            .unwrap();
            assert_ne!(omitted.is_error, Some(true));
            support::assert_initial_scan(
                &serde_json::from_str::<Value>(&omitted.content[0].as_text().unwrap().text)
                    .unwrap()["roms"],
                true,
            );
            for (name, arguments) in read_requests() {
                let result = sdk_call(&client, name, arguments, &diagnostics).await;
                assert_read_payload(name, &sdk_payload(&result));
            }
            let unknown = sdk_call(
                &client,
                "query_metadata",
                json!({"name": "absent"}),
                &diagnostics,
            )
            .await;
            assert_eq!(unknown.is_error, Some(true));
            assert!(
                unknown.content[0]
                    .as_text()
                    .unwrap()
                    .text
                    .contains("was not found")
            );
            if token.is_none() {
                for name in ["move_roms", "delete_roms"] {
                    let destination = fixture.directory.path().join("unauthorized");
                    let result = sdk_call(
                        &client,
                        name,
                        mutation_arguments(name, &destination, Some(false)),
                        &diagnostics,
                    )
                    .await;
                    assert_eq!(result.is_error, Some(true));
                    assert!(
                        result.content[0]
                            .as_text()
                            .unwrap()
                            .text
                            .contains("authorization is required")
                    );
                    assert_eq!(
                        fs::read(fixture.roms.join("pacman.zip")).unwrap(),
                        PACMAN_BYTES
                    );
                    fixture.assert_unrelated_files();
                    assert!(!destination.exists());
                }
                assert!(
                    fs::read_to_string(&diagnostics)
                        .unwrap()
                        .contains("destructive MCP tools are disabled")
                );
            } else {
                for name in ["move_roms", "delete_roms"] {
                    fs::write(fixture.roms.join("pacman.zip"), PACMAN_BYTES).unwrap();
                    let destination = fixture.directory.path().join(format!("sdk-{name}"));
                    if name == "move_roms" {
                        fs::create_dir(&destination).unwrap();
                        fs::write(destination.join("pacman.zip"), b"collision bytes").unwrap();
                        for dry_run in [true, false] {
                            let result = sdk_call(
                                &client,
                                name,
                                mutation_arguments(name, &destination, Some(dry_run)),
                                &diagnostics,
                            )
                            .await;
                            assert_eq!(result.is_error, Some(true));
                            assert!(
                                result.content[0]
                                    .as_text()
                                    .unwrap()
                                    .text
                                    .contains("pacman.zip")
                            );
                            assert_collision_bytes(&fixture, &destination);
                        }
                        fs::remove_file(destination.join("pacman.zip")).unwrap();
                        fs::remove_dir(&destination).unwrap();
                    }
                    for dry_run in [None, Some(true), Some(false)] {
                        if name == "move_roms" && dry_run == Some(false) {
                            fs::create_dir(&destination).unwrap();
                            fs::write(destination.join("notes.txt"), support::NOTE_BYTES).unwrap();
                        }
                        let result = sdk_call(
                            &client,
                            name,
                            mutation_arguments(name, &destination, dry_run),
                            &diagnostics,
                        )
                        .await;
                        assert_eq!(sdk_payload(&result)["roms"], json!(["pacman"]));
                        assert_mutation_files(&fixture, &destination, name, dry_run);
                        let scan = sdk_payload(
                            &sdk_call(&client, "scan_roms", json!({}), &diagnostics).await,
                        );
                        if dry_run == Some(false) {
                            support::assert_rescan(&scan["roms"], true);
                        } else {
                            support::assert_initial_scan(&scan["roms"], true);
                        }
                    }
                }
            }
            tokio::time::timeout(Duration::from_secs(5), client.close())
                .await
                .unwrap()
                .unwrap();
            let status = tokio::time::timeout(Duration::from_secs(5), child.0.wait())
                .await
                .expect("stdio shutdown timed out")
                .unwrap();
            assert!(
                status.success(),
                "stdio exit: {status}\nstderr: {}",
                fs::read_to_string(&diagnostics).unwrap()
            );
        }
    })
    .await
    .expect("stdio SDK interoperability timed out");
}

#[tokio::test]
async fn stdio_process_guard_reaps_the_server_before_removing_test_files() {
    let fixture = FlowFixture::new();
    let diagnostics = fixture.directory.path().join("guard-stderr.txt");
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_arcadejanitor-mcp"));
    for name in support::inherited_application_settings() {
        command.env_remove(name);
    }
    command
        .args(["--transport", "stdio", "--rom-folder"])
        .arg(&fixture.roms)
        .arg("--mame-xml")
        .arg(&fixture.mame_xml)
        .arg("--catver")
        .arg(&fixture.catver)
        .envs(support::environment(fixture.directory.path()))
        .current_dir(fixture.directory.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(File::create(&diagnostics).unwrap())
        .kill_on_drop(true);
    let mut server = StdioProcess(command.spawn().unwrap());
    let transport = (
        server.0.stdout.take().unwrap(),
        server.0.stdin.take().unwrap(),
    );
    let client = match tokio::time::timeout(Duration::from_secs(10), ().serve(transport)).await {
        Ok(Ok(client)) => client,
        Ok(Err(error)) => panic!("stdio guard initialization failed: {error}"),
        Err(error) => panic!("stdio guard initialization timed out: {error}"),
    };
    assert!(server.0.try_wait().unwrap().is_none());
    server.stop_and_reap();
    assert!(
        server.0.try_wait().unwrap().is_some(),
        "stdio child was not reaped"
    );
    drop(server);
    drop(client);
    fixture
        .directory
        .close()
        .expect("stdio test files were not released after cleanup");
}

fn read_requests() -> Vec<(&'static str, Value)> {
    vec![
        ("scan_roms", json!({})),
        ("query_metadata", json!({"name": "PACMAN"})),
        (
            "filter_roms",
            json!({"options": {"categories": ["Maze"], "only_available": false}}),
        ),
        (
            "filter_mame_metadata",
            json!({"options": {"categories": ["Maze"], "only_available": true}}),
        ),
        ("list_catver", json!({"list": "categories"})),
        ("generate_report", json!({})),
    ]
}

fn assert_read_payload(name: &str, payload: &Value) {
    assert_eq!(payload["ok"], true);
    match name {
        "scan_roms" => support::assert_initial_scan(&payload["roms"], true),
        "query_metadata" => {
            assert_eq!(payload["rom"]["name"], "pacman");
            assert!(
                payload["rom"]["rom_path"].is_null(),
                "metadata query is catalog-only"
            );
            assert_eq!(payload["rom"]["metadata"]["region"], "Usa");
            let mut expected = support::expected_collection()[2].clone();
            expected["available"] = json!(false);
            assert_eq!(
                support::normalize_roms(&json!([payload["rom"]]), true),
                json!([expected])
            );
        }
        "filter_roms" => {
            assert_eq!(support::names(&payload["roms"]), ["pacman"]);
            assert_eq!(
                support::normalize_roms(&payload["roms"], true),
                json!([support::expected_collection()[2]])
            );
        }
        "filter_mame_metadata" => {
            assert_eq!(support::names(&payload["roms"]), ["pacman", "missing"]);
            let expected = support::expected_collection();
            let mut rows = json!([expected[1], expected[2]]);
            for row in rows.as_array_mut().unwrap() {
                row["available"] = json!(false);
            }
            assert_eq!(support::normalize_roms(&payload["roms"], true), rows);
        }
        "list_catver" => {
            assert_eq!(payload["list"], "categories");
            assert_eq!(payload["total"], 2);
            assert_eq!(
                support::normalize_categories(&payload["entries"], true),
                support::expected_categories()
            );
        }
        "generate_report" => assert_eq!(
            payload["report"],
            json!({"total": 4, "available": 3, "by_genre": {"Maze": 2, "Shooter": 1, "Unknown": 1}})
        ),
        _ => panic!("unhandled read case {name}"),
    }
}

fn http_payload(response: &Value) -> Value {
    assert!(response["error"].is_null(), "{response}");
    assert_ne!(response["result"]["isError"], true, "{response}");
    serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn sdk_payload(response: &rmcp::model::CallToolResult) -> Value {
    assert_ne!(response.is_error, Some(true), "{response:?}");
    serde_json::from_str(&response.content[0].as_text().unwrap().text).unwrap()
}

async fn sdk_call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    name: &str,
    arguments: Value,
    diagnostics: &Path,
) -> rmcp::model::CallToolResult {
    let request = CallToolRequestParams::new(name.to_string())
        .with_arguments(arguments.as_object().unwrap().clone());
    tokio::time::timeout(Duration::from_secs(5), client.call_tool(request))
        .await
        .unwrap_or_else(|error| {
            panic!(
                "SDK {name} timed out: {error}\nstderr: {}",
                fs::read_to_string(diagnostics).unwrap()
            )
        })
        .unwrap_or_else(|error| {
            panic!(
                "SDK {name} failed: {error}\nstderr: {}",
                fs::read_to_string(diagnostics).unwrap()
            )
        })
}

fn mutation_arguments(name: &str, destination: &Path, dry_run: Option<bool>) -> Value {
    let mut arguments = json!({"filter": {"options": {"names": ["pacman"]}}});
    if name == "move_roms" {
        arguments["target_folder"] = json!(destination);
    }
    if let Some(dry_run) = dry_run {
        arguments["dry_run"] = json!(dry_run);
    }
    arguments
}

fn assert_collision_bytes(fixture: &FlowFixture, destination: &Path) {
    assert_eq!(
        fs::read(fixture.roms.join("pacman.zip")).unwrap(),
        PACMAN_BYTES
    );
    assert_eq!(
        fs::read(destination.join("pacman.zip")).unwrap(),
        b"collision bytes"
    );
    fixture.assert_unrelated_files();
}

fn assert_mutation_files(
    fixture: &FlowFixture,
    destination: &Path,
    name: &str,
    dry_run: Option<bool>,
) {
    if dry_run == Some(false) {
        assert!(!fixture.roms.join("pacman.zip").exists());
        if name == "move_roms" {
            assert_eq!(
                fs::read(destination.join("pacman.zip")).unwrap(),
                PACMAN_BYTES
            );
            assert_eq!(
                fs::read(destination.join("notes.txt")).unwrap(),
                support::NOTE_BYTES
            );
            assert_eq!(fs::read_dir(destination).unwrap().count(), 2);
        } else {
            assert!(!destination.exists());
        }
    } else {
        assert_eq!(
            fs::read(fixture.roms.join("pacman.zip")).unwrap(),
            PACMAN_BYTES
        );
        assert!(!destination.exists());
    }
    fixture.assert_unrelated_files();
}

#[test]
fn populated_http_flows_preview_execute_rescan_and_reject_collisions() {
    let fixture = FlowFixture::new();
    let port = free_port();
    let mut server = start_server_with_token(
        port,
        &fixture.roms,
        &fixture.mame_xml,
        &fixture.catver,
        Some("test-token"),
    );
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .build()
        .unwrap();
    let url = format!("http://127.0.0.1:{port}");
    wait_for_server(&mut server, &client, &url);
    for (name, arguments) in read_requests() {
        assert_read_payload(
            name,
            &http_payload(&http_call(&client, &url, name, arguments, None)),
        );
    }
    let unknown = http_call(
        &client,
        &url,
        "query_metadata",
        json!({"name": "absent"}),
        None,
    );
    assert_eq!(unknown["result"]["isError"], true);
    assert!(
        unknown["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("was not found")
    );
    for name in ["move_roms", "delete_roms"] {
        fs::write(fixture.roms.join("pacman.zip"), PACMAN_BYTES).unwrap();
        let destination = fixture.directory.path().join(format!("http-{name}"));
        if name == "move_roms" {
            fs::create_dir(&destination).unwrap();
            fs::write(destination.join("pacman.zip"), b"collision bytes").unwrap();
            for dry_run in [true, false] {
                let response = http_call(
                    &client,
                    &url,
                    name,
                    mutation_arguments(name, &destination, Some(dry_run)),
                    Some("test-token"),
                );
                assert_eq!(response["result"]["isError"], true);
                assert!(
                    response["result"]["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("pacman.zip")
                );
                assert_collision_bytes(&fixture, &destination);
            }
            fs::remove_file(destination.join("pacman.zip")).unwrap();
            fs::remove_dir(&destination).unwrap();
        }
        for dry_run in [None, Some(true), Some(false)] {
            if name == "move_roms" && dry_run == Some(false) {
                fs::create_dir(&destination).unwrap();
                fs::write(destination.join("notes.txt"), support::NOTE_BYTES).unwrap();
            }
            let payload = http_payload(&http_call(
                &client,
                &url,
                name,
                mutation_arguments(name, &destination, dry_run),
                Some("test-token"),
            ));
            assert_eq!(payload["roms"], json!(["pacman"]));
            assert_mutation_files(&fixture, &destination, name, dry_run);
            let scan = http_payload(&call_tool(&client, &url, "scan_roms", json!({})));
            if dry_run == Some(false) {
                support::assert_rescan(&scan["roms"], true);
            } else {
                support::assert_initial_scan(&scan["roms"], true);
            }
        }
    }
}

#[test]
fn readiness_reports_early_exit_and_rejects_unsuccessful_health_responses() {
    let fixture = FlowFixture::new();
    let client = Client::builder()
        .timeout(Duration::from_millis(100))
        .no_proxy()
        .build()
        .unwrap();
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = occupied.local_addr().unwrap().port();
    let mut failed = start_server(port, &fixture.roms, &fixture.mame_xml, &fixture.catver);
    let error = readiness(
        &mut failed,
        &client,
        &format!("http://127.0.0.1:{port}"),
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert!(error.contains("exited before readiness"), "{error}");
    assert!(error.contains("Error:"), "{error}");
    drop(failed);
    drop(occupied);
    let port = free_port();
    let url = format!("http://127.0.0.1:{port}");
    let mut server = start_server(port, &fixture.roms, &fixture.mame_xml, &fixture.catver);
    wait_for_server(&mut server, &client, &url);
    let start = Instant::now();
    let error = readiness(
        &mut server,
        &client,
        &format!("{url}/invalid"),
        Duration::from_millis(150),
    )
    .unwrap_err();
    assert!(error.contains("404"), "{error}");
    assert!(error.contains("did not become ready"), "{error}");
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(server.child.try_wait().unwrap().is_none());
    drop(server);
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    assert!(TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_err());
}
