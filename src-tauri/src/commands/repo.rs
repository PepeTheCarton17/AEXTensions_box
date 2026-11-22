use crate::github::{GithubApiClient, RepoInfo};
use std::collections::HashMap;

/// Archivos que AEXT genera a nivel de repositorio. Se comprueba su
/// existencia remota uno por uno: si están, son del usuario; si faltan, se
/// suben para que el repo pueda compilar en Actions.
const REPO_SCAFFOLD_FILES: &[&str] = &[
    "README.md",
    "build.gradle.kts",
    "settings.gradle.kts",
    "gradle.properties",
    ".gitignore",
    "gradlew",
    "gradlew.bat",
    "gradle/wrapper/gradle-wrapper.properties",
    "gradle/wrapper/gradle-wrapper.jar",
    ".github/workflows/build_push.yml",
    ".github/scripts/generate-repo-index.py",
    "signingkey.jks",
    "icon/ic_launcher.png",
];

/// Decide, archivo por archivo, qué scaffold hay que subir.
///
/// `repo.empty` no es suficiente: GitHub crea los repos con `auto_init` y un
/// README, así que un repo recién creado parece no vacío aunque no tenga
/// workflow ni wrapper. Al revés, un repo antiguo del usuario puede no tener
/// README propio: en ese caso sí generamos uno mínimo y genérico.
pub(crate) async fn reconcile_repo_files(
    client: &GithubApiClient,
    repo: &RepoInfo,
    bundle: &mut crate::engine::ExtensionBundle,
) -> Result<(), String> {
    for path in REPO_SCAFFOLD_FILES {
        if !is_repo_scaffold(path) {
            return Err(format!("Ruta de scaffold no protegida: {}", path));
        }
        if !client
            .file_exists(&repo.owner, &repo.name, path, &repo.default_branch)
            .await?
        {
            continue; // Falta: se conserva la versión generada.
        }

        match *path {
            "settings.gradle.kts" => {
                // Es el único scaffold que se EDITA: añade el include del
                // módulo nuevo sin borrar los anteriores.
                merge_existing_settings(client, repo, bundle).await?;
            }
            "README.md" => {
                let existing = client
                    .get_file_content(
                        &repo.owner,
                        &repo.name,
                        path,
                        &repo.default_branch,
                    )
                    .await?;
                if !existing
                    .as_deref()
                    .map(|content| is_auto_init_readme(content, &repo.name))
                    .unwrap_or(false)
                {
                    bundle.files.remove(*path);
                }
            }
            _ => {
                // README aparte, el resto es propiedad del usuario: firma,
                // workflow, wrapper y build files nunca se sobrescriben.
                bundle.files.remove(*path);
            }
        }
    }
    Ok(())
}

/// Al reutilizar un repositorio con varias extensiones, añade el `include()`
/// del nuevo módulo sin borrar los que ya existían.
async fn merge_existing_settings(
    client: &GithubApiClient,
    repo: &RepoInfo,
    bundle: &mut crate::engine::ExtensionBundle,
) -> Result<(), String> {
    let existing = client
        .get_file_content(
            &repo.owner,
            &repo.name,
            "settings.gradle.kts",
            &repo.default_branch,
        )
        .await?
        .ok_or_else(|| {
            format!(
                "GitHub informó de settings.gradle.kts en {}/{} pero no devolvió su contenido",
                repo.owner, repo.name
            )
        })?;

    let include = format!("include(\":{}\")", bundle.module_path.replace('/', ":"));
    if existing.contains(&include) {
        bundle.files.remove("settings.gradle.kts");
        return Ok(());
    }

    let mut merged = existing.trim_end().to_string();
    merged.push('\n');
    merged.push_str(&include);
    merged.push('\n');
    bundle
        .files
        .insert("settings.gradle.kts".to_string(), merged.into_bytes());
    Ok(())
}

/// El README de `auto_init` de GitHub contiene solo el título con el nombre
/// del repo y la descripción que AEXT puso al crearlo. Se puede reemplazar
/// por el README mínimo; cualquier otro README se respeta.
fn is_auto_init_readme(content: &str, repo_name: &str) -> bool {
    let lines: Vec<String> = content
        .lines()
        .map(|line| line.trim().to_ascii_lowercase())
        .filter(|line| !line.is_empty())
        .collect();
    let Some(first) = lines.first() else {
        return false;
    };
    if first != &format!("# {}", repo_name.to_ascii_lowercase()) {
        return false;
    }
    lines.len() == 1 || lines.iter().skip(1).any(|line| line.contains("aext"))
}

fn is_repo_scaffold(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower == "readme.md"
        || lower == "readme"
        || lower == "build.gradle.kts"
        || lower == "build.gradle"
        || lower == "settings.gradle.kts"
        || lower == "settings.gradle"
        || lower == "gradle.properties"
        || lower == ".gitignore"
        || lower == "local.properties"
        || lower == "gradlew"
        || lower == "gradlew.bat"
        || lower.starts_with(".github/")
        || lower.starts_with("gradle/")
        || lower == "icon/ic_launcher.png"
        || lower.starts_with("signingkey")
        || lower.ends_with(".jks")
        || lower.ends_with(".keystore")
}

/// Marcadores del scaffold gestionado por AEXT. Un repo existente sólo recibe
/// la versión nueva del workflow/script cuando el contenido anterior lleva una
/// firma reconocida de AEXT: los archivos escritos a mano nunca se tocan.
const MANAGED_WORKFLOW_MARKER: &str = "# aext-managed-workflow";
const LEGACY_WORKFLOW_MARKER: &str = "# No cancelar runs: cada commit de AEXT debe compilarse";
const MANAGED_INDEX_MARKER: &str = "# aext-managed-index";
const LEGACY_INDEX_MARKER: &str = "DEFAULT_FINGERPRINT = ";

fn should_replace_managed_file(path: &str, existing: &str) -> bool {
    match path {
        ".github/workflows/build_push.yml" => {
            existing.contains(MANAGED_WORKFLOW_MARKER) || existing.contains(LEGACY_WORKFLOW_MARKER)
        }
        ".github/scripts/generate-repo-index.py" => {
            existing.contains(MANAGED_INDEX_MARKER)
                || (existing.contains("generate-repo-index") && existing.contains(LEGACY_INDEX_MARKER))
        }
        _ => false,
    }
}

/// Reconcilia el scaffold una sola vez por repo, con todos los archivos del
/// lote ya agregados. `settings.gradle.kts` se resuelve aparte porque debe
/// conservar los `include()` de TODAS las extensiones del grupo.
pub(crate) async fn reconcile_batch_scaffold(
    client: &GithubApiClient,
    repo: &RepoInfo,
    files: &mut HashMap<String, Vec<u8>>,
) -> Result<(), String> {
    for path in REPO_SCAFFOLD_FILES {
        let Some(new_bytes) = files.get(*path).cloned() else {
            continue;
        };
        if !client
            .file_exists(&repo.owner, &repo.name, path, &repo.default_branch)
            .await?
        {
            continue;
        }

        match *path {
            "settings.gradle.kts" => {
                files.remove(*path);
            }
            "README.md" => {
                let existing = client
                    .get_file_content(&repo.owner, &repo.name, path, &repo.default_branch)
                    .await?;
                if !existing
                    .as_deref()
                    .map(|content| is_auto_init_readme(content, &repo.name))
                    .unwrap_or(false)
                {
                    files.remove(*path);
                }
            }
            ".github/workflows/build_push.yml" | ".github/scripts/generate-repo-index.py" => {
                let existing = client
                    .get_file_content(&repo.owner, &repo.name, path, &repo.default_branch)
                    .await?
                    .unwrap_or_default();
                if existing.as_bytes() == new_bytes.as_slice()
                    || !should_replace_managed_file(path, &existing)
                {
                    files.remove(*path);
                }
            }
            _ => {
                files.remove(*path);
            }
        }
    }
    Ok(())
}

/// Construye un `settings.gradle.kts` con un `include()` por cada módulo del
/// lote. Se usa el contenido remoto como base; si el repo no lo tiene, se parte
/// del generado por AEXT (o de un esqueleto mínimo).
pub(crate) async fn settings_for_modules(
    client: &GithubApiClient,
    repo: &RepoInfo,
    module_paths: &[String],
    generated: Option<Vec<u8>>,
) -> Result<Option<Vec<u8>>, String> {
    let path = "settings.gradle.kts";
    let exists = client
        .file_exists(&repo.owner, &repo.name, path, &repo.default_branch)
        .await?;
    let base = if exists {
        client
            .get_file_content(&repo.owner, &repo.name, path, &repo.default_branch)
            .await?
            .map(String::into_bytes)
    } else {
        None
    };
    let original = base.or(generated);
    let mut settings = match original {
        Some(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        None => "rootProject.name = \"nekoyomi_extensions\"\n".to_string(),
    };
    let before = settings.clone();

    for module in module_paths {
        let include = format!("include(\":{}\")", module.replace('/', ":"));
        if settings.contains(&include) {
            continue;
        }
        if !settings.ends_with('\n') {
            settings.push('\n');
        }
        settings.push_str(&include);
        settings.push('\n');
    }

    if exists && settings == before {
        return Ok(None);
    }
    Ok(Some(settings.into_bytes()))
}

#[cfg(test)]
mod scaffold_tests {
    use super::{is_auto_init_readme, is_repo_scaffold, REPO_SCAFFOLD_FILES};

    #[test]
    fn scaffold_list_covers_every_protected_root_file() {
        for path in REPO_SCAFFOLD_FILES {
            assert!(
                is_repo_scaffold(path),
                "el scaffold debe proteger archivos del repo: {}",
                path
            );
        }
    }

    #[test]
    fn detects_auto_init_readme_only() {
        let placeholder = "# nekoyomi-ext-generated\nRepositorio de extensiones Nekoyomi generado automáticamente por AEXT\n\n";
        assert!(is_auto_init_readme(placeholder, "nekoyomi-ext-generated"));
        assert!(is_auto_init_readme("# MiRepo\n", "mirepo"));
        assert!(!is_auto_init_readme(
            "# MiRepo\nDocumentación del proyecto\n",
            "mirepo"
        ));
        assert!(!is_auto_init_readme(
            "# Otro repo\nGenerado por AEXT\n",
            "mirepo"
        ));
    }

    #[test]
    fn protects_user_owned_repo_files() {
        for path in [
            "README.md",
            "readme",
            "signingkey.jks",
            "build.gradle.kts",
            "settings.gradle.kts",
            "gradle.properties",
            "gradlew",
            "gradlew.bat",
            "gradle/wrapper/gradle-wrapper.properties",
            "gradle/wrapper/gradle-wrapper.jar",
            ".github/workflows/build_push.yml",
            ".github/scripts/generate-repo-index.py",
            ".gitignore",
            "icon/ic_launcher.png",
        ] {
            assert!(is_repo_scaffold(path), "debe protegerse: {}", path);
        }
    }

    #[test]
    fn allows_extension_owned_files() {
        for path in [
            "src/es/miextension/build.gradle.kts",
            "src/es/miextension/src/main/AndroidManifest.xml",
            "src/es/miextension/src/main/java/eu/kanade/MiExt.kt",
            "src/es/miextension/src/main/res/mipmap-hdpi/ic_launcher.png",
            "icon/eu.kanade.tachiyomi.animeextension.es.miextension.png",
        ] {
            assert!(!is_repo_scaffold(path), "debe poder subirse: {}", path);
        }
    }
}
