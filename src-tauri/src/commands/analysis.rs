use crate::engine::{AnalysisResult, SiteAnalyzer};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn analyze_url(url: String, state: State<'_, AppState>) -> Result<AnalysisResult, String> {
    state
        .add_log("INFO", &format!("Iniciando análisis de URL: {}", url))
        .await;
    let analyzer = SiteAnalyzer::new();
    match analyzer.analyze(&url).await {
        Ok(mut res) => {
            // Cuando el catálogo HTML no se verificó, AEXT prueba los motores
            // disponibles con peticiones anónimas antes de mostrar el análisis.
            crate::commands::engines::run_engine_trials(&state, &mut res).await;
            state
                .add_log(
                    "SUCCESS",
                    &format!(
                        "Análisis completado para {}: CMS={}, Tipo={}",
                        res.name, res.cms, res.ext_type
                    ),
                )
                .await;
            Ok(res)
        }
        Err(e) => {
            state
                .add_log("ERROR", &format!("Error analizando {}: {}", url, e))
                .await;
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn analyze_multiple_urls(
    urls: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<AnalysisResult>, String> {
    let mut results = Vec::new();
    let analyzer = SiteAnalyzer::new();

    for url in urls {
        state.add_log("INFO", &format!("Analizando: {}", url)).await;
        if let Ok(mut res) = analyzer.analyze(&url).await {
            crate::commands::engines::run_engine_trials(&state, &mut res).await;
            results.push(res);
        }
    }

    Ok(results)
}
