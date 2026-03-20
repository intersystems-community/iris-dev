use rmcp::{ServerHandler, model::*, tool, tool_handler, tool_router,
           handler::server::{router::tool::ToolRouter, wrapper::Parameters},
           service::RequestContext, RoleServer, ErrorData as McpError};
use serde::Deserialize;
use schemars::JsonSchema;
use std::sync::Arc;
use crate::iris::connection::IrisConnection;

/// All 23 iris-dev MCP tools live here.
/// Pattern: #[tool_router] impl IrisTools + #[tool_handler] impl ServerHandler
#[derive(Clone)]
pub struct IrisTools {
    pub iris: Option<Arc<IrisConnection>>,
    tool_router: ToolRouter<IrisTools>,
}

// --- Input schemas ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompileParams {
    /// Class name (e.g. "MyApp.Patient") or path to .cls file
    pub target: String,
    #[serde(default = "default_flags")]
    pub flags: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default)]
    pub force_writable: bool,
}
fn default_flags() -> String { "cuk".to_string() }
fn default_namespace() -> String { "USER".to_string() }

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SymbolsParams {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_limit() -> usize { 20 }

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugMapParams {
    #[serde(default)]
    pub routine: String,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub error_string: String,
}

fn iris_unreachable() -> McpError {
    McpError::invalid_request("IRIS_UNREACHABLE: no IRIS connection available", None)
}

#[tool_router]
impl IrisTools {
    pub fn new(iris: Option<IrisConnection>) -> Self {
        Self {
            iris: iris.map(Arc::new),
            tool_router: Self::tool_router(),
        }
    }

    /// Compile an ObjectScript class or .cls file on IRIS
    #[tool(description = "Compile an ObjectScript class or .cls file on IRIS. Pass a class name (e.g. MyApp.Patient) or path to a .cls file. Returns compile errors with line numbers.")]
    async fn iris_compile(
        &self,
        Parameters(p): Parameters<CompileParams>,
    ) -> Result<CallToolResult, McpError> {
        let iris = self.iris.as_ref().ok_or_else(iris_unreachable)?;
        // TODO: POST /api/atelier/v1/{ns}/action/xecute with $SYSTEM.OBJ.Load()
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "success": false,
                "error_code": "NOT_IMPLEMENTED",
                "target": p.target,
                "namespace": iris.namespace,
            }).to_string()
        )]))
    }

    /// Map a .INT routine offset to the original .CLS source line
    #[tool(description = "Map a .INT routine offset from an IRIS error stack back to the .CLS source line. Pass routine+offset or a raw error string like '<UNDEFINED>x+3^MyApp.Foo.1'.")]
    async fn debug_map_int_to_cls(
        &self,
        Parameters(p): Parameters<DebugMapParams>,
    ) -> Result<CallToolResult, McpError> {
        // TODO: parse error_string, call %Studio.Debugger.SourceLine, check .int.map sidecar
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "success": false,
                "error_code": "NOT_IMPLEMENTED",
                "routine": p.routine,
                "offset": p.offset,
            }).to_string()
        )]))
    }

    /// Search for ObjectScript symbols in the current workspace or IRIS namespace
    #[tool(description = "Search for ObjectScript classes, methods, and properties. Searches the workspace .cls files offline, or queries IRIS %Dictionary if connected.")]
    async fn iris_symbols(
        &self,
        Parameters(p): Parameters<SymbolsParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "success": false,
                "error_code": "NOT_IMPLEMENTED",
                "query": p.query,
            }).to_string()
        )]))
    }
}

#[tool_handler]
impl ServerHandler for IrisTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder().enable_tools().build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_instructions("iris-dev MCP server: 23 tools for ObjectScript and IRIS development. Compile, test, introspect, debug, and manage skills/knowledge for AI-assisted IRIS development.".to_string())
    }
}
