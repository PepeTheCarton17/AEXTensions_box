use crate::engine::generator::{
    decode_base64, GRADLEW_BAT, GRADLEW_SH, GRADLE_WRAPPER_JAR_B64, GRADLE_WRAPPER_PROPERTIES,
    ROOT_BUILD_GRADLE,
};
use std::fs;
use std::path::Path;

/// Busca los `build.gradle.kts` de los módulos bajo `src/`.
fn find_module_build_files(root: &Path) -> Vec<std::path::PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.file_name().and_then(|n| n.to_str()) == Some("build.gradle.kts") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("src"), &mut out);
    out
}

/// Actualiza el toolchain de un proyecto generado por una versión anterior de
/// AEXT (AGP 8.2.2/Gradle 8.5) a la combinación actual (AGP 9/Gradle 9.5.1).
/// No toca el código Kotlin: el usuario puede haber personalizado los
/// extractores. Sin esto, una carpeta antigua sigue fallando aunque se instale
/// una versión nueva de AEXT.
pub(crate) fn upgrade_project_toolchain(dir: &Path) -> Result<(), String> {
    // 1. Wrapper: lo sobrescribe siempre, es propiedad de AEXT.
    let wrapper_dir = dir.join("gradle").join("wrapper");
    fs::create_dir_all(&wrapper_dir)
        .map_err(|e| format!("No se pudo crear {}: {}", wrapper_dir.display(), e))?;
    fs::write(
        wrapper_dir.join("gradle-wrapper.properties"),
        GRADLE_WRAPPER_PROPERTIES.as_bytes(),
    )
    .map_err(|e| format!("No se pudo actualizar gradle-wrapper.properties: {}", e))?;
    fs::write(
        wrapper_dir.join("gradle-wrapper.jar"),
        decode_base64(GRADLE_WRAPPER_JAR_B64),
    )
    .map_err(|e| format!("No se pudo actualizar gradle-wrapper.jar: {}", e))?;
    fs::write(dir.join("gradlew"), GRADLEW_SH.as_bytes())
        .map_err(|e| format!("No se pudo actualizar gradlew: {}", e))?;
    fs::write(dir.join("gradlew.bat"), GRADLEW_BAT.as_bytes())
        .map_err(|e| format!("No se pudo actualizar gradlew.bat: {}", e))?;

    // 2. build.gradle.kts raíz (no lleva código del usuario).
    fs::write(dir.join("build.gradle.kts"), ROOT_BUILD_GRADLE.as_bytes())
        .map_err(|e| format!("No se pudo actualizar build.gradle.kts: {}", e))?;

    // 3. build.gradle.kts del módulo: AGP 9 rechaza `kotlin("android")` y ya no
    //    expone `kotlinOptions`; hay que migrar al DSL `compilerOptions`.
    const NEW_KOTLIN: &str = "    kotlin {\n        compilerOptions {\n            jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_1_8\n            freeCompilerArgs.add(\"-opt-in=kotlinx.serialization.ExperimentalSerializationApi\")\n        }\n    }";
    for build_file in find_module_build_files(dir) {
        let text = fs::read_to_string(&build_file).unwrap_or_default();
        let mut out: String = text
            .lines()
            .filter(|line| !line.contains("kotlin(\"android\")"))
            .map(|line| format!("{}\n", line))
            .collect();
        if let Some(start) = out.find("kotlinOptions {") {
            let line_start = out[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let mut depth = 0i32;
            let mut end = None;
            for (i, ch) in out[start..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(start + i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(end) = end {
                out.replace_range(line_start..end, NEW_KOTLIN);
            }
        }
        // El lint de release de AGP 9 intenta resolver dependencias transitivas
        // muertas del extensions-lib antiguo (injekt-core de JitPack) y rompe
        // `assembleRelease`. Las extensiones no necesitan lint vital.
        if !out.contains("checkReleaseBuilds") {
            if let Some(pos) = out.find("compileOptions {") {
                let insert = "    lint {\n        checkReleaseBuilds = false\n    }\n\n";
                out.insert_str(pos, insert);
            }
        }
        fs::write(&build_file, out)
            .map_err(|e| format!("No se pudo actualizar {}: {}", build_file.display(), e))?;
    }
    Ok(())
}
