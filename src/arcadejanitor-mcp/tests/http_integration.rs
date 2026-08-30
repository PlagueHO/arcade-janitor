use std::{
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::Duration,
};

use reqwest::blocking::Client;
use serde_json::{Value, json};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn start_server(port: u16) -> Child {
    Command::new(env!("CARGO_BIN_EXE_arcadejanitor-mcp"))
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
    let mut server = start_server(port);
    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let url = format!("http://127.0.0.1:{port}");
    wait_for_server(&client, &url);

    let response = call_tool(
        &client,
        &url,
        "scan_roms",
        json!({
            "rom_folder": roms,
            "mame_xml": fixture("mame-100.xml"),
            "catver": fixture("catver-100.ini")
        }),
    );
    assert_eq!(response["result"]["isError"], Value::Null);
    let content = response["result"]["content"][0]["text"].as_str().unwrap();
    let payload: Value = serde_json::from_str(content).unwrap();
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["roms"].as_array().unwrap().len(), 101);
    assert_eq!(payload["roms"][0]["name"], "game001");

    server.kill().unwrap();
    server.wait().unwrap();
}
