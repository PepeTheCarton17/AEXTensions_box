//! Lectura y validación de `aext-engine.json`.
//!
//! La validación es la frontera de seguridad del registro: exige licencia y
//! proveniencia explícitas y rechaza cualquier sonda que pida cookies,
//! autorización o tokens de sesión. AEXT no actúa como sesión del usuario.

use super::model::{EngineDescriptor, EngineManifest};

const FORBIDDEN_HEADERS: [&str; 6] = [
    "cookie",
    "set-cookie",
    "authorization",
    "proxy-authorization",
    "x-csrf-token",
    "x-xsrf-token",
];

const FORBIDDEN_MARKERS: [&str; 3] = ["{cookie}", "{session}", "{token}"];

const EXPECTATIONS: [&str; 3] = ["json_list", "html_items", "hls"];

pub fn parse(raw: &str) -> Result<EngineDescriptor, String> {
    let manifest: EngineManifest = serde_json::from_str(raw)
        .map_err(|e| format!("aext-engine.json no es JSON válido: {}", e))?;
    validate(&manifest)?;
    Ok(manifest.engine)
}

fn validate(manifest: &EngineManifest) -> Result<(), String> {
    if manifest.version != 1 {
        return Err(format!(
            "manifest_version {} no soportada (se espera 1)",
            manifest.version
        ));
    }
    let engine = &manifest.engine;
    if engine.id.is_empty()
        || !engine
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err("id debe usar minúsculas, dígitos, '-' o '_'".to_string());
    }
    if engine.name.trim().is_empty() {
        return Err("name no puede estar vacío".to_string());
    }
    if engine.capabilities.is_empty() {
        return Err("capabilities no puede estar vacío".to_string());
    }
    if engine.license.trim().is_empty() {
        return Err("license es obligatoria (no se acepta 'unknown')".to_string());
    }
    let lower_license = engine.license.to_lowercase();
    if lower_license == "unknown" || lower_license == "none" || lower_license == "propietario" {
        return Err("license debe ser explícita (p. ej. Apache-2.0, MIT)".to_string());
    }
    if !engine.provenance.starts_with("http://")
        && !engine.provenance.starts_with("https://")
        && !engine.provenance.starts_with("builtin:")
    {
        return Err("provenance debe ser una URL o 'builtin:aext'".to_string());
    }
    if engine.probes.is_empty() {
        return Err("probes no puede estar vacío: sin sonda no hay prueba".to_string());
    }

    validate_files(&engine.files)?;
    if let Some(runtime) = &engine.runtime {
        runtime.validate(&engine.files, &engine.kind)?;
    }
    for probe in &engine.probes {
        let method = probe.method.to_uppercase();
        if method != "GET" && method != "POST" {
            return Err(format!("sonda {}: método {} no soportado", probe.id, method));
        }
        if !EXPECTATIONS.contains(&probe.expect.as_str()) {
            return Err(format!(
                "sonda {}: expect '{}' no soportado",
                probe.id, probe.expect
            ));
        }
        if probe.url_template.trim().is_empty() {
            return Err(format!("sonda {}: url_template vacío", probe.id));
        }
        if !probe.url_template.contains("{base}")
            && !probe.url_template.contains("{endpoint}")
            && !probe.url_template.contains("{stream}")
            && !probe.url_template.starts_with("http://")
            && !probe.url_template.starts_with("https://")
        {
            return Err(format!(
                "sonda {}: la URL debe ser absoluta o usar {{base}}, {{endpoint}} o {{stream}}",
                probe.id
            ));
        }
        let mut templates = vec![probe.url_template.as_str()];
        if let Some(body) = &probe.body_template {
            templates.push(body.as_str());
        }
        if templates.iter().any(|t| {
            let lower = t.to_lowercase();
            FORBIDDEN_MARKERS.iter().any(|m| lower.contains(m))
        }) {
            return Err(format!(
                "sonda {}: no se permiten marcadores de cookie/sesión/token",
                probe.id
            ));
        }
        for (name, _) in &probe.headers {
            if FORBIDDEN_HEADERS.contains(&name.to_lowercase().as_str()) {
                return Err(format!(
                    "sonda {}: la cabecera '{}' está prohibida (AEXT no usa sesión)",
                    probe.id, name
                ));
            }
        }
        if probe.expect == "html_items" && probe.item_selector.is_none() {
            return Err(format!(
                "sonda {}: html_items necesita item_selector",
                probe.id
            ));
        }
    }
    Ok(())
}

fn validate_files(files: &[String]) -> Result<(), String> {
    for file in files {
        if file.starts_with('/')
            || file.split('/').any(|part| part == ".." || part.is_empty())
        {
            return Err(format!("ruta de archivo no segura: {}", file));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn manifest(headers: &str, url: &str) -> String {
        format!(
            r#"{{
              "version": 1,
              "id": "engine-demo",
              "name": "Demo",
              "kind": "catalog",
              "capabilities": ["json-catalog"],
              "source": "repo",
              "license": "MIT",
              "provenance": "https://github.com/owner/repo",
              "probes": [{{
                "id": "list",
                "url_template": "{url}",
                "expect": "json_list",
                "headers": {headers}
              }}]
            }}"#
        )
    }

    #[test]
    fn accepts_a_declarative_manifest() {
        assert!(parse(&manifest("[]", "{endpoint}")).is_ok());
    }

    #[test]
    fn rejects_session_headers_and_markers() {
        let with_cookie = manifest(r#"[["cookie", "sid=1"]]"#, "{endpoint}");
        assert!(parse(&with_cookie).is_err());
        let with_marker = manifest("[]", "{endpoint}?s={session}");
        assert!(parse(&with_marker).is_err());
    }

    #[test]
    fn rejects_missing_license_or_provenance() {
        let raw = manifest("[]", "{endpoint}").replace("\"MIT\"", "\"unknown\"");
        assert!(parse(&raw).is_err());
        let raw = manifest("[]", "{endpoint}").replace(
            "\"provenance\": \"https://github.com/owner/repo\"",
            "\"provenance\": \"\"",
        );
        assert!(parse(&raw).is_err());
    }

    fn runtime_manifest(runtime: &str, files: &str) -> String {
        format!(
            r#"{{
              "version": 1,
              "id": "engine-demo",
              "name": "Demo",
              "kind": "playback",
              "capabilities": ["direct-http"],
              "source": "repo",
              "license": "MIT",
              "provenance": "https://github.com/owner/repo",
              "probes": [{{
                "id": "probe",
                "url_template": "{{endpoint}}",
                "expect": "json_list"
              }}],
              "files": {files},
              "runtime": {runtime}
            }}"#
        )
    }

    fn valid_runtime() -> &'static str {
        r#"{
          "api": "aext-stream-resolver/1",
          "package": "com.example.engine",
          "entry_class": "DemoResolver"
        }"#
    }

    #[test]
    fn accepts_a_declared_runtime() {
        let raw = runtime_manifest(valid_runtime(), r#"["src/DemoResolver.kt"]"#);
        let engine = parse(&raw).expect("runtime válido");
        let runtime = engine.runtime.expect("runtime declarado");
        assert_eq!(runtime.entry_class, "DemoResolver");
    }

    #[test]
    fn rejects_runtime_with_unknown_api_or_missing_kotlin_file() {
        let bad_api = valid_runtime().replace("aext-stream-resolver/1", "aext-stream-resolver/2");
        assert!(parse(&runtime_manifest(&bad_api, r#"["src/DemoResolver.kt"]"#)).is_err());
        assert!(parse(&runtime_manifest(valid_runtime(), r#"["README.md"]"#)).is_err());
    }
}
