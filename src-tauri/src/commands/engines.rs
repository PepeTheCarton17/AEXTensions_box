//! Comandos para instalar, listar y probar motores de forma anónima.

use crate::engine::analyzer::{AnalysisResult, ApiListResult, SiteAnalyzer};
use crate::engine::registry::{self, EngineDescriptor, EngineSource, EngineTrial, ProbeContext};
use crate::state::AppState;
use std::fs;
use std::time::Duration;
use tauri::State;

fn probe_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("AEXT-EngineProbe/1.0")
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(12))
        .build()
        .unwrap_or_default()
}

fn pick_field(candidates: &[&str], keys: &[String]) -> String {
    candidates
        .iter()
        .find(|candidate| keys.iter().any(|key| key.eq_ignore_ascii_case(*candidate)))
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn detect_fields(sample: &str) -> (String, String, String) {
    let value: serde_json::Value = match serde_json::from_str(sample) {
        Ok(value) => value,
        Err(_) => return (String::new(), String::new(), String::new()),
    };
    let array = if value.is_array() {
        value.as_array()
    } else {
        value
            .as_object()
            .and_then(|map| map.values().find_map(|item| item.as_array()))
    };
    let keys: Vec<String> = array
        .and_then(|items| items.first())
        .and_then(|item| item.as_object())
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    (
        pick_field(&["title", "name", "label"], &keys),
        pick_field(&["image", "poster", "poster_link", "thumbnail"], &keys),
        pick_field(&["url", "link", "permalink", "href", "id"], &keys),
    )
}

fn sample_to_api_list(trial: &EngineTrial, sample: &str) -> ApiListResult {
    let (title_field, image_field, url_field) = detect_fields(sample);
    // `content` es una etiqueta genérica que el generador ya entiende como
    // catálogo. Sólo se usa cuando el JSON trae título y enlace reales; si no,
    // la muestra queda como dato sin clasificar y nunca se vuelve tarjeta.
    let resource = if !title_field.is_empty() && !url_field.is_empty() {
        "content".to_string()
    } else {
        trial.probe_id.clone()
    };
    ApiListResult {
        url: trial.url.clone(),
        resource,
        options: Vec::new(),
        sample_json: sample.to_string(),
        title_field,
        image_field,
        url_field,
        record_count: trial.content_count,
    }
}

/// Prueba los motores compatibles con lo que el análisis vio realmente. Las
/// muestras verificadas pasan a `api_lists`; el generador las consume con sus
/// plantillas genéricas, sin código de proveedor.
pub(crate) async fn run_engine_trials(state: &AppState, analysis: &mut AnalysisResult) {
    let engines = state.engines.read().await.clone();
    let catalog_missing = analysis.catalog_source == "none" || !analysis.catalog_detected;
    let selected = registry::select(&engines, &analysis.capabilities, catalog_missing);
    if selected.is_empty() {
        state
            .add_log("INFO", "Sin motores compatibles para este sitio.")
            .await;
        return;
    }

    let mut endpoints = analysis.capabilities.api_endpoints.clone();
    for api in &analysis.api_lists {
        if !endpoints.contains(&api.url) {
            endpoints.push(api.url.clone());
        }
    }
    let streams: Vec<String> = endpoints
        .iter()
        .filter(|url| url.contains(".m3u8"))
        .cloned()
        .collect();
    let context = ProbeContext {
        base: analysis.url.clone(),
        endpoints,
        streams,
    };

    let http = probe_client();
    let mut trials = Vec::new();
    for engine in &selected {
        let attempted = registry::probe_engine(&http, engine, &context).await;
        state
            .add_log(
                "INFO",
                &format!(
                    "Motor '{}' ({}) probado: {} sonda(s).",
                    engine.name,
                    engine.license,
                    attempted.len()
                ),
            )
            .await;
        trials.extend(attempted);
    }

    for trial in &trials {
        if !trial.success {
            continue;
        }
        if let Some(sample) = &trial.sample_json {
            let api = sample_to_api_list(trial, sample);
            if !analysis.api_lists.iter().any(|old| old.url == api.url) {
                analysis.api_lists.push(api);
            }
        }
    }

    let successes: Vec<&EngineTrial> = trials.iter().filter(|trial| trial.success).collect();
    if successes.is_empty() {
        analysis.warnings.push(
            "Se probaron motores compatibles, pero ninguno devolvió contenido verificable sin sesión. La extensión no afirma catálogo funcional."
                .to_string(),
        );
        state
            .add_log("WARN", "Ningún motor devolvió contenido verificable.")
            .await;
    } else {
        let names: Vec<String> = successes
            .iter()
            .map(|trial| format!("{} ({})", trial.engine_name, trial.license))
            .collect();
        let names = names.join(", ");
        analysis.warnings.push(format!(
            "Motores con contenido real verificado: {}. Las muestras se usan como catálogo del sitio.",
            names
        ));
        state
            .add_log(
                "SUCCESS",
                &format!("Motores con contenido verificado: {}.", names),
            )
            .await;
    }
    analysis.engine_trials = trials;
}

#[tauri::command]
pub async fn list_engines(state: State<'_, AppState>) -> Result<Vec<EngineDescriptor>, String> {
    Ok(state.engines.read().await.clone())
}

#[tauri::command]
pub async fn install_engine(
    repo_url: String,
    state: State<'_, AppState>,
) -> Result<EngineDescriptor, String> {
    state
        .add_log("INFO", &format!("Instalando motor desde {}", repo_url))
        .await;
    let (engine, repo) = registry::fetch(&repo_url).await?;
    let dir = registry::save_files(&state.storage_dir, &engine, &repo).await?;
    {
        let mut engines = state.engines.write().await;
        registry::upsert(&mut engines, engine.clone());
    }
    state.persist_engines().await;
    state
        .add_log(
            "SUCCESS",
            &format!(
                "Motor '{}' instalado ({}, {}). Archivos en {}.",
                engine.name,
                engine.license,
                engine.provenance,
                dir.display()
            ),
        )
        .await;
    Ok(engine)
}

#[tauri::command]
pub async fn set_engine_enabled(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<Vec<EngineDescriptor>, String> {
    {
        let mut engines = state.engines.write().await;
        if !registry::disable(&mut engines, &id, !enabled) {
            return Err(format!("No existe el motor '{}'", id));
        }
    }
    state.persist_engines().await;
    Ok(state.engines.read().await.clone())
}

#[tauri::command]
pub async fn uninstall_engine(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<EngineDescriptor>, String> {
    let builtin = state
        .engines
        .read()
        .await
        .iter()
        .find(|engine| engine.id == id)
        .map(|engine| engine.source == EngineSource::Builtin)
        .unwrap_or(false);
    if builtin {
        return Err("Los motores incluidos no se desinstalan: puedes desactivarlos.".to_string());
    }
    {
        let mut engines = state.engines.write().await;
        engines.retain(|engine| engine.id != id);
    }
    let dir = registry::install_dir(&state.storage_dir, &id);
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    state.persist_engines().await;
    state
        .add_log("WARN", &format!("Motor '{}' desinstalado.", id))
        .await;
    Ok(state.engines.read().await.clone())
}

#[tauri::command]
pub async fn test_engines(
    url: String,
    state: State<'_, AppState>,
) -> Result<Vec<EngineTrial>, String> {
    state
        .add_log("INFO", &format!("Probando motores para {}", url))
        .await;
    let analyzer = SiteAnalyzer::new();
    let mut analysis = analyzer.analyze(&url).await?;
    run_engine_trials(&state, &mut analysis).await;
    Ok(analysis.engine_trials)
}
