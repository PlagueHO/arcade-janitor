use std::{net::SocketAddr, path::PathBuf};

use axum::{
    Json, Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use cleanmame_core::operations::{
    delete::delete_roms,
    filter::{FilterOptions, filter_roms},
    r#move::move_roms,
    query::{find_by_name, load_metadata, scan_rom_folder},
    report::generate_report,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_stream::{self as stream, StreamExt};
use tracing::info;

#[derive(Clone)]
struct AppState;

#[derive(Debug, Deserialize)]
struct ToolRequest {
    tool: String,
    #[serde(default)]
    args: Value,
}

#[derive(Debug, Serialize)]
struct ToolDefinition {
    name: &'static str,
    description: &'static str,
    input_schema: Value,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cleanmame_core::utils::logging::init_logging();
    let addr: SocketAddr = std::env::var("CLEANMAME_MCP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()?;

    let app = Router::new()
        .route(
            "/health",
            get(|| async { format!("ok ({})", mcp_sdk_name()) }),
        )
        .route("/tools", get(list_tools))
        .route("/tools/call", post(call_tool))
        .route("/ws", get(websocket_handler))
        .with_state(AppState);

    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "CleanMAME MCP server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn list_tools() -> Json<Vec<ToolDefinition>> {
    Json(stream::iter(tools()).collect::<Vec<_>>().await)
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(message)) = socket.recv().await {
        let Message::Text(text) = message else {
            continue;
        };
        let response = match serde_json::from_str::<ToolRequest>(&text) {
            Ok(request) => execute_tool(request),
            Err(error) => json!({ "ok": false, "error": error.to_string() }),
        };
        if socket
            .send(Message::Text(response.to_string().into()))
            .await
            .is_err()
        {
            break;
        }
    }
}

async fn call_tool(Json(request): Json<ToolRequest>) -> impl IntoResponse {
    let response = execute_tool(request);
    let status = if response.get("ok").and_then(Value::as_bool) == Some(true) {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(response))
}

fn execute_tool(request: ToolRequest) -> Value {
    match request.tool.as_str() {
        "scan_roms" => scan_roms(request.args),
        "query_metadata" => query_metadata(request.args),
        "filter_roms" => filter_roms_tool(request.args),
        "move_roms" => move_roms_tool(request.args),
        "delete_roms" => delete_roms_tool(request.args),
        "generate_report" => generate_report_tool(request.args),
        _ => json!({ "ok": false, "error": format!("unknown tool '{}'", request.tool) }),
    }
}

fn scan_roms(args: Value) -> Value {
    match parse_metadata_args(args).and_then(|args| {
        scan_rom_folder(args.rom_folder, args.mame_xml, args.catver).map_err(anyhow::Error::from)
    }) {
        Ok(roms) => json!({ "ok": true, "roms": roms }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    }
}

fn query_metadata(args: Value) -> Value {
    match parse_query_args(args).and_then(|args| {
        let roms = load_metadata(args.mame_xml, args.catver.as_deref())?;
        find_by_name(&roms, &args.name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("ROM '{}' was not found", args.name))
    }) {
        Ok(rom) => json!({ "ok": true, "rom": rom }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    }
}

fn filter_roms_tool(args: Value) -> Value {
    match parse_filter_args(args).and_then(|args| {
        let roms = scan_rom_folder(
            args.metadata.rom_folder,
            args.metadata.mame_xml,
            args.metadata.catver,
        )?;
        Ok::<_, anyhow::Error>(filter_roms(&roms, &args.options))
    }) {
        Ok(roms) => json!({ "ok": true, "roms": roms }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    }
}

fn move_roms_tool(args: Value) -> Value {
    match parse_mutating_args(args).and_then(|args| {
        let roms = scan_rom_folder(
            args.filter.metadata.rom_folder,
            args.filter.metadata.mame_xml,
            args.filter.metadata.catver,
        )?;
        let filtered = filter_roms(&roms, &args.filter.options);
        move_roms(&filtered, args.target_folder, args.dry_run).map_err(anyhow::Error::from)
    }) {
        Ok(moved) => json!({ "ok": true, "roms": moved }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    }
}

fn delete_roms_tool(args: Value) -> Value {
    match parse_delete_args(args).and_then(|args| {
        let roms = scan_rom_folder(
            args.filter.metadata.rom_folder,
            args.filter.metadata.mame_xml,
            args.filter.metadata.catver,
        )?;
        let filtered = filter_roms(&roms, &args.filter.options);
        delete_roms(&filtered, args.dry_run).map_err(anyhow::Error::from)
    }) {
        Ok(deleted) => json!({ "ok": true, "roms": deleted }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    }
}

fn generate_report_tool(args: Value) -> Value {
    match parse_metadata_args(args).and_then(|args| {
        scan_rom_folder(args.rom_folder, args.mame_xml, args.catver).map_err(anyhow::Error::from)
    }) {
        Ok(roms) => json!({ "ok": true, "report": generate_report(&roms) }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    }
}

fn parse_metadata_args(args: Value) -> anyhow::Result<MetadataArgs> {
    serde_json::from_value(args).map_err(Into::into)
}

fn parse_query_args(args: Value) -> anyhow::Result<QueryArgs> {
    serde_json::from_value(args).map_err(Into::into)
}

fn parse_filter_args(args: Value) -> anyhow::Result<FilterArgs> {
    serde_json::from_value(args).map_err(Into::into)
}

fn parse_mutating_args(args: Value) -> anyhow::Result<MoveArgs> {
    serde_json::from_value(args).map_err(Into::into)
}

fn parse_delete_args(args: Value) -> anyhow::Result<DeleteArgs> {
    serde_json::from_value(args).map_err(Into::into)
}

fn tools() -> Vec<ToolDefinition> {
    vec![
        tool(
            "scan_roms",
            "Scan a ROM folder using mame.xml and optional catver.ini",
        ),
        tool("query_metadata", "Query metadata for one ROM name"),
        tool("filter_roms", "Filter available ROMs by metadata"),
        tool("move_roms", "Move filtered ROM files to a target folder"),
        tool("delete_roms", "Delete filtered ROM files"),
        tool("generate_report", "Generate a simple metadata report"),
    ]
}

fn tool(name: &'static str, description: &'static str) -> ToolDefinition {
    ToolDefinition {
        name,
        description,
        input_schema: json!({
            "type": "object",
            "additionalProperties": true
        }),
    }
}

fn mcp_sdk_name() -> &'static str {
    std::any::type_name::<mcp_server_rs::server::Server<()>>()
}

#[derive(Debug, Deserialize)]
struct MetadataArgs {
    rom_folder: PathBuf,
    mame_xml: PathBuf,
    catver: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct QueryArgs {
    name: String,
    mame_xml: PathBuf,
    catver: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct FilterArgs {
    metadata: MetadataArgs,
    #[serde(default)]
    options: FilterOptions,
}

#[derive(Debug, Deserialize)]
struct MoveArgs {
    filter: FilterArgs,
    target_folder: PathBuf,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Debug, Deserialize)]
struct DeleteArgs {
    filter: FilterArgs,
    #[serde(default)]
    dry_run: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lists_required_tools() {
        let tools = list_tools().await.0;

        assert!(tools.iter().any(|tool| tool.name == "scan_roms"));
        assert!(tools.iter().any(|tool| tool.name == "generate_report"));
    }
}
