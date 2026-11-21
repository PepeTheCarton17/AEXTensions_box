use crate::commands::build_upgrade::upgrade_project_toolchain;
use crate::commands::files::local_extension_dir;
use crate::commands::toolchain::{build_tool_command, detect_toolchain};
use crate::state::AppState;
use std::fs;
use std::path::Path;
use tauri::State;

/// Actualiza el estado de compilación que se muestra en "Mis extensiones".
async fn set_build_status(
    state: &AppState,
    pkg_id: &str,
    status: &str,
    error: Option<String>,
    apk_path: Option<String>,
) {
    {
        let mut list = state.extensions.write().await;
        if let Some(ext) = list
            .iter_mut()
            .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
        {
            ext.build_status = Some(status.to_string());
            if status == "building" {
                ext.build_error = None;
                ext.build_apk_path = None;
            } else {
                ext.build_error = error;
                ext.build_apk_path = apk_path;
            }
        }
    }
    state.persist_extensions().await;
}

/// Extrae las líneas útiles de Gradle para la terminal (en vez de un genérico
/// "La compilación falló").
fn extract_build_error(output: &str) -> String {
    let mut picked: Vec<String> = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_lowercase();
        let relevant = line.starts_with("e: ")
            || lower.contains("error:")
            || lower.contains("failure:")
            || lower.contains("what went wrong")
            || lower.contains("execution failed")
            || lower.contains("cannot mutate")
            || lower.contains("unresolved reference");
        if relevant && !picked.iter().any(|p| p == line) {
            picked.push(line.to_string());
        }
        if picked.len() >= 4 {
            break;
        }
    }
    if picked.is_empty() {
        "Gradle devolvió un error sin mensaje detallado. Revisa la salida completa abajo."
            .to_string()
    } else {
        picked.join("\n")
    }
}

/// Elimina APKs de compilaciones anteriores para no dar un éxito falso cuando
/// Gradle falla pero el APK viejo sigue en `build/outputs`.
fn remove_stale_apks(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            remove_stale_apks(&path);
        } else if path.extension() == Some(std::ffi::OsStr::new("apk")) {
            let _ = fs::remove_file(path);
        }
    }
}

/// Compila el módulo con Gradle + Android SDK si están instalados. Es una
/// compilación real (firma incluida); nunca simulamos un APK inexistente.
#[tauri::command]
pub async fn compile_extension(
    state: State<'_, AppState>,
    pkg_id: String,
) -> Result<serde_json::Value, String> {
    let app_state = state.inner().clone();
    let dir = local_extension_dir(&state, &pkg_id);
    if !dir.is_dir() {
        let msg = format!(
            "No hay código local para compilar. Descarga el ZIP o regenera la extensión ({}).",
            dir.display()
        );
        set_build_status(&app_state, &pkg_id, "error", Some(msg.clone()), None).await;
        return Err(msg);
    }
    if !dir.join("settings.gradle.kts").exists() {
        let msg = "El proyecto local no tiene settings.gradle.kts. Regenera la extensión con esta versión de AEXT.".to_string();
        set_build_status(&app_state, &pkg_id, "error", Some(msg.clone()), None).await;
        return Err(msg);
    }

    // Un proyecto creado con una versión anterior de AEXT sigue trayendo
    // AGP 8.2.2 + Gradle 8.5, que no arranca con las JDK 25/27 actuales.
    // Migrar los archivos de compilación sin tocar el código Kotlin.
    if let Err(e) = upgrade_project_toolchain(&dir) {
        set_build_status(&app_state, &pkg_id, "error", Some(e.clone()), None).await;
        return Err(e);
    }

    let toolchain = tauri::async_runtime::spawn_blocking(detect_toolchain)
        .await
        .map_err(|e| e.to_string())?;

    if !toolchain.java || !toolchain.android_sdk {
        let msg = format!(
            "Faltan herramientas para compilar: {}",
            toolchain.missing.join(", ")
        );
        set_build_status(&app_state, &pkg_id, "error", Some(msg.clone()), None).await;
        return Err(msg);
    }

    let parts: Vec<&str> = pkg_id.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        let msg = "No se pudo deducir la ruta del módulo Gradle.".to_string();
        set_build_status(&app_state, &pkg_id, "error", Some(msg.clone()), None).await;
        return Err(msg);
    }
    let module = format!(
        ":src:{}:{}",
        parts[parts.len() - 2],
        parts[parts.len() - 1]
    );

    // Preferir SIEMPRE el wrapper del proyecto: fija Gradle 8.5. El Gradle del
    // PATH puede ser 9.x y romper AGP 8.2.2.
    let wrapper = if cfg!(windows) {
        dir.join("gradlew.bat")
    } else {
        dir.join("gradlew")
    };
    #[cfg(unix)]
    if wrapper.is_file() {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755));
    }
    let gradle = if wrapper.is_file() {
        wrapper.display().to_string()
    } else {
        let detected = toolchain.gradle_path.clone().ok_or_else(|| {
            "No se encontró Gradle ni un wrapper en el proyecto. Regenera la extensión."
                .to_string()
        })?;
        detected
    };
    let sdk = toolchain
        .android_sdk_path
        .clone()
        .ok_or_else(|| "No se encontró el Android SDK.".to_string())?;
    let java_home = toolchain.java_home.clone();

    set_build_status(&app_state, &pkg_id, "building", None, None).await;

    // local.properties apunta al SDK real del usuario.
    let local_properties = format!("sdk.dir={}\n", sdk.replace('\\', "/"));
    fs::write(dir.join("local.properties"), local_properties)
        .map_err(|e| format!("No se pudo escribir local.properties: {}", e))?;

    // Un APK anterior no debe hacer que un build fallido parezca exitoso.
    remove_stale_apks(&dir.join("src"));

    app_state
        .add_log(
            "INFO",
            &format!(
                "Compilando {}:assembleRelease (puede tardar varios minutos)…",
                module
            ),
        )
        .await;

    let build_dir = dir.clone();
    let build_module = module.clone();
    let build_sdk = sdk.clone();
    let build_java = java_home.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut command = build_tool_command(&gradle);
        command
            .arg("--no-daemon")
            .arg("--console=plain")
            .arg(format!("{}:assembleRelease", build_module))
            .current_dir(&build_dir)
            .env("ANDROID_HOME", &build_sdk)
            .env("ANDROID_SDK_ROOT", &build_sdk);
        if let Some(home) = build_java {
            command.env("JAVA_HOME", home);
        }
        command.output()
    })
    .await
    .map_err(|e| format!("La compilación se interrumpió: {}", e))?;

    let output = result.map_err(|e| {
        format!(
            "No se pudo ejecutar Gradle ({}). Verifica que Java, Gradle y el Android SDK estén instalados.",
            e
        )
    })?;

    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push('\n');
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    let tail: Vec<&str> = combined.lines().rev().take(160).collect();
    let tail = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
    let error_summary = if output.status.success() {
        None
    } else {
        Some(extract_build_error(&combined))
    };

    let apk = find_apk(&dir.join("src"));
    let success = output.status.success() && apk.is_some();
    let apk_path = apk.map(|p| p.display().to_string());

    if success {
        set_build_status(&app_state, &pkg_id, "success", None, apk_path.clone()).await;
        app_state
            .add_log(
                "SUCCESS",
                &format!("APK compilado en {}", apk_path.clone().unwrap_or_default()),
            )
            .await;
    } else {
        let detail = error_summary
            .clone()
            .unwrap_or_else(|| "Sin detalles de Gradle.".to_string());
        set_build_status(&app_state, &pkg_id, "error", Some(detail.clone()), None).await;
        app_state
            .add_log(
                "ERROR",
                &format!("La compilación de {} falló:\n{}", pkg_id, detail),
            )
            .await;
    }

    Ok(serde_json::json!({
        "success": success,
        "apk_path": apk_path,
        "exit_code": output.status.code(),
        "error": error_summary,
        "output": tail
    }))
}

fn find_apk(root: &Path) -> Option<std::path::PathBuf> {
    if !root.is_dir() {
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    let mut best: Option<std::path::PathBuf> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_apk(&path) {
                let newer = fs::metadata(&found)
                    .and_then(|m| m.modified())
                    .ok();
                let current = best.as_ref().and_then(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
                if best.is_none() || newer > current {
                    best = Some(found);
                }
            }
        } else if path.extension().map(|ext| ext == "apk").unwrap_or(false) {
            best = Some(path);
        }
    }
    best
}
