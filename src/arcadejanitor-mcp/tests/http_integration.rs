use std::{
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use reqwest::blocking::Client;
use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest};

struct ServerProcess(Child);

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.0.kill().expect("stop MCP test server");
        self.0.wait().expect("reap MCP test server");
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn start_server(port: u16, rom_folder: &Path, mame_xml: &Path, catver: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_arcadejanitor-mcp"))
        .args([
            "--rom-folder",
            rom_folder.to_str().unwrap(),
            "--mame-xml",
            mame_xml.to_str().unwrap(),
            "--catver",
            catver.to_str().unwrap(),
        ])
        .env("ARCADEJANITOR_MCP_ADDR", format!("127.0.0.1:{port}"))
        .spawn()
        .unwrap()
}

fn wait_for_server(client: &Client, url: &str) {
    for _ in 0..50 {
        if client.get(format!("{url}/health")).send().is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("MCP server did not become ready");
}

fn call_tool(client: &Client, url: &str, name: &str, arguments: Value) -> Value {
    client
        .post(format!("{url}/mcp"))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }))
        .send()
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
    let _server = ServerProcess(start_server(port, &roms, &mame_xml, &catver));
    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let url = format!("http://127.0.0.1:{port}");
    wait_for_server(&client, &url);

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
    }

    for arguments in [Value::Null, json!([1]), json!("invalid")] {
        let response = call_tool(&client, &url, "scan_roms", arguments);
        assert_eq!(response["result"]["isError"], true);
    }
}

#[test]
fn preserves_stateless_http_protocol_and_authorization() {
    let directory = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcadejanitor-mcp"));
    let child = command
        .arg("--rom-folder")
        .arg(directory.path())
        .arg("--mame-xml")
        .arg(fixture("mame-100.xml"))
        .arg("--catver")
        .arg(fixture("catver-100.ini"))
        .env("ARCADEJANITOR_MCP_ADDR", format!("127.0.0.1:{port}"))
        .env("ARCADEJANITOR_MCP_TOKEN", "test-token")
        .spawn()
        .unwrap();
    let _server = ServerProcess(child);
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let url = format!("http://127.0.0.1:{port}");
    wait_for_server(&client, &url);
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
    match tungstenite::connect(untrusted).unwrap_err() {
        tungstenite::Error::Http(response) => assert_eq!(response.status(), 403),
        error => panic!("unexpected WebSocket rejection: {error}"),
    }
    let mut trusted = ws_url.as_str().into_client_request().unwrap();
    trusted
        .headers_mut()
        .insert("Origin", "http://localhost:3000".parse().unwrap());
    trusted
        .headers_mut()
        .insert("Authorization", "Bearer test-token".parse().unwrap());
    let (mut socket, _) = tungstenite::connect(trusted).unwrap();
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
    }
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

#[tokio::test]
async fn serves_stdio_to_the_official_sdk_client() {
    tokio::time::timeout(Duration::from_secs(15), async {
        for token in [None, Some("test-token")] {
            let directory = tempfile::tempdir().unwrap();
            let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_arcadejanitor-mcp"));
            command
                .arg("--transport")
                .arg("stdio")
                .arg("--rom-folder")
                .arg(directory.path())
                .arg("--mame-xml")
                .arg(fixture("mame-100.xml"))
                .arg("--catver")
                .arg(fixture("catver-100.ini"))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .env("RUST_LOG", "arcadejanitor_mcp=info")
                .kill_on_drop(true);
            match token {
                Some(token) => {
                    command.env("ARCADEJANITOR_MCP_TOKEN", token);
                }
                None => {
                    command.env_remove("ARCADEJANITOR_MCP_TOKEN");
                }
            }
            let mut child = command.spawn().unwrap();
            let transport = (child.stdout.take().unwrap(), child.stdin.take().unwrap());
            let mut client = ().serve(transport).await.unwrap();
            let tools = client.list_tools(None).await.unwrap();
            assert_eq!(tools.tools.len(), if token.is_some() { 9 } else { 7 });
            for request in [
                CallToolRequestParams::new("scan_roms"),
                CallToolRequestParams::new("scan_roms").with_arguments(rmcp::object!({})),
            ] {
                let result = client.call_tool(request).await.unwrap();
                assert_ne!(result.is_error, Some(true));
                let text = result.content[0].as_text().unwrap();
                let payload: Value = serde_json::from_str(&text.text).unwrap();
                assert_eq!(payload["ok"], true);
                assert_eq!(payload["roms"].as_array().unwrap().len(), 100);
            }
            let result = client
                .call_tool(
                    CallToolRequestParams::new("delete_roms")
                        .with_arguments(rmcp::object!({"filter": {}, "dry_run": true})),
                )
                .await
                .unwrap();
            assert_eq!(result.is_error == Some(true), token.is_none());
            client.close().await.unwrap();
            let output = child.wait_with_output().await.unwrap();
            assert!(output.status.success());
            let diagnostics = String::from_utf8(output.stderr).unwrap();
            if token.is_none() {
                assert!(diagnostics.contains("destructive MCP tools are disabled"));
            }
        }
    })
    .await
    .expect("stdio SDK interoperability timed out");
}
