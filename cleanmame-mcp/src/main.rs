use std::{future::Future, net::SocketAddr, path::PathBuf, pin::Pin, sync::Arc};

use axum::{
    Json, Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use cleanmame_core::operations::{
    delete::delete_roms,
    filter::{FilterOptions, filter_roms},
    r#move::move_roms,
    query::{find_by_name, load_metadata, scan_rom_folder},
    report::generate_report,
};
use mcp_core_rs::{
    Resource, Tool,
    content::Content,
    prompt::Prompt,
    protocol::{
        capabilities::ServerCapabilities,
        error::ErrorData,
        message::{JsonRpcRequest, JsonRpcResponse},
    },
};
use mcp_error_rs::{Error as McpError, Result as McpResult};
use mcp_server_rs::router::{capabilities::CapabilitiesBuilder, traits::Router as McpRouter};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tracing::{info, warn};

const SERVER_NAME: &str = "cleanmame";

#[derive(Clone)]
struct AppState {
    auth_token: Option<Arc<str>>,
}

#[derive(Clone)]
struct CleanMameRouter {
    destructive_tools_allowed: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cleanmame_core::utils::logging::init_logging();
    let addr: SocketAddr = std::env::var("CLEANMAME_MCP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()?;
    let state = AppState {
        auth_token: std::env::var("CLEANMAME_MCP_TOKEN")
            .ok()
            .filter(|token| !token.is_empty())
            .map(Arc::<str>::from),
    };
    if state.auth_token.is_none() {
        warn!("destructive MCP tools are disabled; set CLEANMAME_MCP_TOKEN to enable them");
    }

    let app = Router::new()
        .route("/health", get(|| async { format!("ok ({SERVER_NAME})") }))
        .route("/mcp", post(mcp_handler))
        .route("/ws", get(websocket_handler))
        .with_state(state);

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

async fn websocket_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !has_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let router = router_for_request(&state, &headers);
    ws.on_upgrade(move |socket| handle_socket(socket, router))
        .into_response()
}

async fn handle_socket(mut socket: WebSocket, router: CleanMameRouter) {
    while let Some(Ok(message)) = socket.recv().await {
        let Message::Text(text) = message else {
            continue;
        };
        let response = match serde_json::from_str::<WireRequest>(&text) {
            Ok(request) => handle_wire_request(router.clone(), request).await,
            Err(error) => Some(wire_error(None, -32700, error.to_string())),
        };
        if let Some(response) = response
            && socket
                .send(Message::Text(
                    serde_json::to_string(&response).unwrap_or_default().into(),
                ))
                .await
                .is_err()
        {
            break;
        }
    }
}

async fn mcp_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    let request = match serde_json::from_value::<WireRequest>(request) {
        Ok(request) => request,
        Err(error) => return Json(wire_error(None, -32600, error.to_string())).into_response(),
    };
    match handle_wire_request(router_for_request(&state, &headers), request).await {
        Some(response) => Json(response).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

fn router_for_request(state: &AppState, headers: &HeaderMap) -> CleanMameRouter {
    CleanMameRouter {
        destructive_tools_allowed: is_authorized(state, headers),
    }
}

fn is_authorized(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(token) = &state.auth_token else {
        return false;
    };
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|provided| provided == token.as_ref())
}

fn has_trusted_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    [
        "http://localhost",
        "https://localhost",
        "http://127.0.0.1",
        "https://127.0.0.1",
    ]
    .iter()
    .any(|base| {
        origin == *base
            || origin.strip_prefix(base).is_some_and(|suffix| {
                suffix.starts_with(':') && suffix[1..].bytes().all(|byte| byte.is_ascii_digit())
            })
    })
}

async fn handle_request(
    router: CleanMameRouter,
    request: JsonRpcRequest,
) -> Option<JsonRpcResponse> {
    let id = request.id?;
    let result = match request.method.as_str() {
        "initialize" => router.handle_initialize(request).await,
        "tools/list" => router.handle_tools_list(request).await,
        "tools/call" => router.handle_tools_call(request).await,
        _ => return Some(rpc_error(Some(id), -32601, "method not found")),
    };
    match result {
        Ok(response) => Some(response),
        Err(McpError::InvalidParameters(message)) => Some(rpc_error(Some(id), -32602, message)),
        Err(error) => Some(rpc_error(Some(id), -32603, error.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct WireRequest {
    jsonrpc: String,
    id: Option<WireId>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
enum WireId {
    Number(u64),
    String(String),
}

#[derive(Debug, Serialize)]
struct WireResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<WireId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorData>,
}

async fn handle_wire_request(
    router: CleanMameRouter,
    request: WireRequest,
) -> Option<WireResponse> {
    let id = request.id.clone()?;
    if request.method == "ping" {
        return Some(WireResponse {
            jsonrpc: request.jsonrpc,
            id: Some(id),
            result: Some(json!({})),
            error: None,
        });
    }

    let sdk_id = match &id {
        WireId::Number(id) => *id,
        WireId::String(_) => 0,
    };
    let response = handle_request(
        router,
        JsonRpcRequest::new(Some(sdk_id), request.method, request.params),
    )
    .await?;
    Some(WireResponse {
        jsonrpc: response.jsonrpc,
        id: Some(id),
        result: response.result,
        error: response.error,
    })
}

fn wire_error(id: Option<WireId>, code: i32, message: impl Into<String>) -> WireResponse {
    WireResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(ErrorData {
            code,
            message: message.into(),
            data: None,
        }),
    }
}

fn rpc_error(id: Option<u64>, code: i32, message: impl Into<String>) -> JsonRpcResponse {
    JsonRpcResponse::error(
        id,
        ErrorData {
            code,
            message: message.into(),
            data: None,
        },
    )
}

impl McpRouter for CleanMameRouter {
    fn name(&self) -> String {
        SERVER_NAME.to_string()
    }

    fn instructions(&self) -> String {
        "Scan, query, filter, move, delete, and report on MAME ROMs.".to_string()
    }

    fn capabilities(&self) -> ServerCapabilities {
        CapabilitiesBuilder::new().with_tools(false).build()
    }

    fn list_tools(&self) -> Vec<Tool> {
        tools()
            .into_iter()
            .filter(|tool| self.destructive_tools_allowed || !is_destructive_tool(&tool.name))
            .collect()
    }

    fn call_tool(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Pin<Box<dyn Future<Output = McpResult<Vec<Content>>> + Send + 'static>> {
        let tool_name = tool_name.to_string();
        let destructive_tools_allowed = self.destructive_tools_allowed;
        Box::pin(async move {
            if is_destructive_tool(&tool_name) && !destructive_tools_allowed {
                return Err(McpError::Protocol(
                    "authorization is required for destructive tools".to_string(),
                ));
            }
            let response = tokio::task::spawn_blocking(move || execute_tool(&tool_name, arguments))
                .await
                .map_err(|error| McpError::System(error.to_string()))?;
            if response.get("ok").and_then(Value::as_bool) == Some(true) {
                Ok(vec![Content::text(response.to_string())])
            } else {
                Err(McpError::System(
                    response
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("tool execution failed")
                        .to_string(),
                ))
            }
        })
    }

    fn list_resources(&self) -> Vec<Resource> {
        Vec::new()
    }

    fn read_resource(
        &self,
        _uri: &str,
    ) -> Pin<Box<dyn Future<Output = McpResult<String>> + Send + 'static>> {
        Box::pin(async {
            Err(McpError::Protocol(
                "resources are not supported".to_string(),
            ))
        })
    }

    fn list_prompts(&self) -> Vec<Prompt> {
        Vec::new()
    }

    fn get_prompt(
        &self,
        _prompt_name: &str,
    ) -> Pin<Box<dyn Future<Output = McpResult<String>> + Send + 'static>> {
        Box::pin(async { Err(McpError::Protocol("prompts are not supported".to_string())) })
    }
}

fn is_destructive_tool(tool_name: &str) -> bool {
    matches!(tool_name, "move_roms" | "delete_roms")
}

fn execute_tool(tool_name: &str, args: Value) -> Value {
    match tool_name {
        "scan_roms" => scan_roms(args),
        "query_metadata" => query_metadata(args),
        "filter_roms" => filter_roms_tool(args),
        "move_roms" => move_roms_tool(args),
        "delete_roms" => delete_roms_tool(args),
        "generate_report" => generate_report_tool(args),
        _ => json!({ "ok": false, "error": format!("unknown tool '{tool_name}'") }),
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
        let mut options = args.options;
        options.only_available = true;
        let roms = scan_rom_folder(
            args.metadata.rom_folder,
            args.metadata.mame_xml,
            args.metadata.catver,
        )?;
        Ok::<_, anyhow::Error>(filter_roms(&roms, &options))
    }) {
        Ok(roms) => json!({ "ok": true, "roms": roms }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    }
}

fn move_roms_tool(args: Value) -> Value {
    match parse_move_args(args).and_then(|args| {
        let mut options = args.filter.options;
        options.only_available = true;
        let roms = scan_rom_folder(
            args.filter.metadata.rom_folder,
            args.filter.metadata.mame_xml,
            args.filter.metadata.catver,
        )?;
        let filtered = filter_roms(&roms, &options);
        move_roms(&filtered, args.target_folder, args.dry_run).map_err(anyhow::Error::from)
    }) {
        Ok(moved) => json!({ "ok": true, "roms": moved }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    }
}

fn delete_roms_tool(args: Value) -> Value {
    match parse_delete_args(args).and_then(|args| {
        let mut options = args.filter.options;
        options.only_available = true;
        let roms = scan_rom_folder(
            args.filter.metadata.rom_folder,
            args.filter.metadata.mame_xml,
            args.filter.metadata.catver,
        )?;
        let filtered = filter_roms(&roms, &options);
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

fn parse_move_args(args: Value) -> anyhow::Result<MoveArgs> {
    serde_json::from_value(args).map_err(Into::into)
}

fn parse_delete_args(args: Value) -> anyhow::Result<DeleteArgs> {
    serde_json::from_value(args).map_err(Into::into)
}

fn tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "scan_roms",
            "Scan a ROM folder using mame.xml and optional catver.ini",
            metadata_schema(),
        ),
        Tool::new(
            "query_metadata",
            "Query metadata for one ROM name",
            query_schema(),
        ),
        Tool::new(
            "filter_roms",
            "Filter available ROMs by metadata",
            filter_schema(),
        ),
        Tool::new(
            "move_roms",
            "Move filtered ROM files to a target folder",
            move_schema(),
        ),
        Tool::new("delete_roms", "Delete filtered ROM files", delete_schema()),
        Tool::new(
            "generate_report",
            "Generate a simple metadata report",
            metadata_schema(),
        ),
    ]
}

fn metadata_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "rom_folder": { "type": "string" },
            "mame_xml": { "type": "string" },
            "catver": { "type": ["string", "null"] }
        },
        "required": ["rom_folder", "mame_xml"],
        "additionalProperties": false
    })
}

fn query_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "mame_xml": { "type": "string" },
            "catver": { "type": ["string", "null"] }
        },
        "required": ["name", "mame_xml"],
        "additionalProperties": false
    })
}

fn filter_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "metadata": metadata_schema(),
            "options": filter_options_schema()
        },
        "required": ["metadata"],
        "additionalProperties": false
    })
}

fn filter_options_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "genre_contains": { "type": ["string", "null"] },
            "region": {
                "oneOf": [
                    { "enum": ["Usa", "Japan", "Europe", "World", "Asia", "Unknown"] },
                    {
                        "type": "object",
                        "properties": { "Other": { "type": "string" } },
                        "required": ["Other"],
                        "additionalProperties": false
                    },
                    { "type": "null" }
                ]
            },
            "include_mature": { "type": "boolean" },
            "include_mechanical": { "type": "boolean" },
            "include_prototype": { "type": "boolean" },
            "only_available": { "type": "boolean" }
        },
        "additionalProperties": false
    })
}

fn move_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "filter": filter_schema(),
            "target_folder": { "type": "string" },
            "dry_run": { "type": "boolean" }
        },
        "required": ["filter", "target_folder"],
        "additionalProperties": false
    })
}

fn delete_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "filter": filter_schema(),
            "dry_run": { "type": "boolean" }
        },
        "required": ["filter"],
        "additionalProperties": false
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataArgs {
    rom_folder: PathBuf,
    mame_xml: PathBuf,
    catver: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryArgs {
    name: String,
    mame_xml: PathBuf,
    catver: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilterArgs {
    metadata: MetadataArgs,
    #[serde(default)]
    options: FilterOptions,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveArgs {
    filter: FilterArgs,
    target_folder: PathBuf,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteArgs {
    filter: FilterArgs,
    #[serde(default)]
    dry_run: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lists_required_tools_with_schemas_and_preserves_request_id() {
        let response = handle_request(
            CleanMameRouter {
                destructive_tools_allowed: true,
            },
            JsonRpcRequest::new(Some(42), "tools/list", None),
        )
        .await;
        let response = response.unwrap();
        let tools = response.result.unwrap()["tools"]
            .as_array()
            .unwrap()
            .clone();

        assert_eq!(response.id, Some(42));
        let scan = tools
            .iter()
            .find(|tool| tool["name"] == "scan_roms")
            .unwrap();
        assert_eq!(
            scan["inputSchema"]["required"],
            json!(["rom_folder", "mame_xml"])
        );
        assert!(tools.iter().any(|tool| tool["name"] == "generate_report"));
    }

    #[tokio::test]
    async fn responds_accepted_to_notifications() {
        let response = mcp_handler(
            State(AppState { auth_token: None }),
            HeaderMap::new(),
            Json(json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[test]
    fn destructive_tools_only_select_available_roms() {
        let folder = std::env::temp_dir().join(format!("cleanmame-mcp-{}", std::process::id()));
        let rom_folder = folder.join("roms");
        let target_folder = folder.join("target");
        let mame_xml = folder.join("mame.xml");
        std::fs::create_dir_all(&rom_folder).unwrap();
        std::fs::write(rom_folder.join("available.zip"), "").unwrap();
        std::fs::write(
            &mame_xml,
            r#"<mame><machine name="available"></machine><machine name="missing"></machine></mame>"#,
        )
        .unwrap();

        let response = execute_tool(
            "move_roms",
            json!({
                "filter": {
                    "metadata": {
                        "rom_folder": rom_folder,
                        "mame_xml": mame_xml
                    }
                },
                "target_folder": target_folder,
                "dry_run": true
            }),
        );

        assert_eq!(response, json!({ "ok": true, "roms": ["available"] }));
        let response = execute_tool(
            "delete_roms",
            json!({
                "filter": {
                    "metadata": {
                        "rom_folder": rom_folder,
                        "mame_xml": mame_xml
                    }
                },
                "dry_run": true
            }),
        );

        assert_eq!(response, json!({ "ok": true, "roms": ["available"] }));
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn requires_bearer_authentication_for_destructive_tools() {
        let state = AppState {
            auth_token: Some(Arc::from("token")),
        };
        let mut headers = HeaderMap::new();

        assert!(!router_for_request(&state, &headers).destructive_tools_allowed);
        assert!(
            router_for_request(&state, &headers)
                .list_tools()
                .iter()
                .all(|tool| !is_destructive_tool(&tool.name))
        );
        headers.insert(
            header::AUTHORIZATION,
            ["Bearer", "token"].join(" ").parse().unwrap(),
        );
        assert!(router_for_request(&state, &headers).destructive_tools_allowed);
    }

    #[test]
    fn only_accepts_local_websocket_origins() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, "https://example.com".parse().unwrap());
        assert!(!has_trusted_origin(&headers));

        headers.insert(header::ORIGIN, "http://localhost:3000".parse().unwrap());
        assert!(has_trusted_origin(&headers));
    }
}
