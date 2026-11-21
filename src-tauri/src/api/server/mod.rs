//! Servidor API & MCP embebido.
//!
//! El antiguo archivo único se reparte por responsabilidad: arranque y rutas,
//! descargas, diagnóstico, parches, utilidades de archivos y protocolo MCP.
//! Las rutas HTTP, los payloads y las firmas públicas no cambian.

mod diagnostics;
mod extensions;
mod files;
mod mcp;
mod models;
mod patch;
mod routes;

pub use models::AnalyzePayload;
pub use routes::start_embedded_api_server;

/// Versión de la app, compartida por el estado, los diagnósticos y el MCP.
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
