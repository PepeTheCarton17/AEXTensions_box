//! Tipos del registro genérico de motores.
//!
//! Un motor es una descripción declarativa: qué capacidades aporta, cómo se
//! prueba y de dónde viene. AEXT no conoce dominios ni nombres de proveedores;
//! sólo lee manifiestos y ejecuta las sondas que éstos declaran.

use super::runtime::EngineRuntime;
use serde::{Deserialize, Serialize};

fn default_version() -> u32 {
    1
}

fn default_get() -> String {
    "GET".to_string()
}

fn default_json_list() -> String {
    "json_list".to_string()
}

fn default_min_items() -> usize {
    1
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineKind {
    Catalog,
    Playback,
    Extractor,
}

impl Default for EngineKind {
    fn default() -> Self {
        Self::Extractor
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSource {
    Builtin,
    Repo,
}

impl Default for EngineSource {
    fn default() -> Self {
        Self::Repo
    }
}

/// Una sonda HTTP declarada por el manifiesto. Los marcadores `{base}`,
/// `{endpoint}` y `{stream}` se sustituyen con datos reales detectados en el
/// sitio: AEXT nunca inventa rutas ni escribe URLs de un proveedor.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineProbe {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_get")]
    pub method: String,
    #[serde(default)]
    pub url_template: String,
    #[serde(default)]
    pub body_template: Option<String>,
    /// Cabeceras declaradas por el motor. Nunca se permiten cookies ni
    /// credenciales: AEXT no usa la sesión del usuario (ver `manifest.rs`).
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// `json_list` | `html_items` | `hls`.
    #[serde(default = "default_json_list")]
    pub expect: String,
    #[serde(default)]
    pub list_field: Option<String>,
    #[serde(default)]
    pub item_selector: Option<String>,
    #[serde(default = "default_min_items")]
    pub min_items: usize,
}

/// Motor instalado o incluido en AEXT.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineDescriptor {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: EngineKind,
    /// Etiquetas de capacidad (`json-catalog`, `hls`, `cipher-signature`...).
    /// El router sólo compara estas etiquetas; nunca dominios.
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub source: EngineSource,
    #[serde(default)]
    pub license: String,
    /// Repositorio de origen o `builtin:aext`. Se guarda con el motor para que
    /// sobreviva a actualizaciones y se pueda auditar.
    #[serde(default)]
    pub provenance: String,
    #[serde(default)]
    pub probes: Vec<EngineProbe>,
    /// Archivos declarados en el repo, copiados bajo `<storage>/engines/<id>/`.
    #[serde(default)]
    pub files: Vec<String>,
    /// Contrato Kotlin que este motor aporta al bundle generado. `None` si el
    /// motor sólo participa en sondas y catálogo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<EngineRuntime>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Resultado de probar una sonda. Es la única prueba de que un motor "da
/// contenido": AEXT no afirma funcionabilidad sin este resultado.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineTrial {
    #[serde(default)]
    pub engine_id: String,
    #[serde(default)]
    pub engine_name: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub provenance: String,
    #[serde(default)]
    pub probe_id: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub content_count: usize,
    #[serde(default)]
    pub detail: String,
    /// Muestra recortada de un JSON verificado. Se convierte en catálogo real
    /// para el generador, sin código específico de proveedor.
    #[serde(default)]
    pub sample_json: Option<String>,
}

/// Sobre el manifiesto que publica un repositorio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineManifest {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(flatten)]
    pub engine: EngineDescriptor,
}
