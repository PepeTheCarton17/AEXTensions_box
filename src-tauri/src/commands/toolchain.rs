use std::path::Path;

#[derive(serde::Serialize)]
pub struct BuildToolchain {
    pub os: String,
    pub java: bool,
    pub java_path: Option<String>,
    pub java_version: Option<String>,
    pub java_home: Option<String>,
    pub kotlinc: bool,
    pub kotlinc_version: Option<String>,
    pub gradle: bool,
    pub gradle_path: Option<String>,
    pub gradle_version: Option<String>,
    pub android_sdk: bool,
    pub android_sdk_path: Option<String>,
    pub can_build: bool,
    pub missing: Vec<String>,
    pub notes: Vec<String>,
}

#[tauri::command]
pub async fn check_build_toolchain() -> Result<BuildToolchain, String> {
    tauri::async_runtime::spawn_blocking(detect_toolchain)
        .await
        .map_err(|e| e.to_string())
}

pub(crate) fn build_tool_command(program: &str) -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let lower = program.to_ascii_lowercase();
        if lower.ends_with(".bat") || lower.ends_with(".cmd") {
            let mut command = std::process::Command::new("cmd");
            command.arg("/C").arg(program);
            command.creation_flags(CREATE_NO_WINDOW);
            return command;
        }
        let mut command = std::process::Command::new(program);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new(program)
    }
}

fn which_tool(program: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("where");
        command.arg(program).creation_flags(0x0800_0000);
        command.output().ok()?
    };
    #[cfg(not(target_os = "windows"))]
    let output = std::process::Command::new("which")
        .arg(program)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}

fn run_version(program: &str, args: &[&str]) -> Option<String> {
    let output = build_tool_command(program).args(args).output().ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if text.trim().is_empty() {
        text = stderr;
    }
    text.lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}

fn find_file_under(root: &Path, file_name: &str, depth: usize) -> Option<std::path::PathBuf> {
    if depth == 0 || !root.is_dir() {
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .map(|name| name.to_string_lossy().as_ref() == file_name)
                .unwrap_or(false)
        {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_file_under(&path, file_name, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
}

pub(crate) fn detect_toolchain() -> BuildToolchain {
    let mut missing = Vec::new();
    let mut notes = Vec::new();

    // Java (el JBR de Android Studio también sirve).
    let java_from_home = std::env::var_os("JAVA_HOME").map(std::path::PathBuf::from).and_then(|home| {
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        let candidate = home.join("bin").join(exe);
        if candidate.is_file() {
            Some((candidate.display().to_string(), home.display().to_string()))
        } else {
            None
        }
    });

    let studio_java = [
        "C:/Program Files/Android/Android Studio/jbr",
        "/Applications/Android Studio.app/Contents/jbr/Contents/Home",
        "/opt/android-studio/jbr",
    ]
    .iter()
    .find_map(|base| {
        let home = std::path::PathBuf::from(base);
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        let candidate = home.join("bin").join(exe);
        if candidate.is_file() {
            Some((candidate.display().to_string(), home.display().to_string()))
        } else {
            None
        }
    });

    let java_path = java_from_home
        .clone()
        .map(|(path, _)| path)
        .or_else(|| which_tool("java"))
        .or_else(|| studio_java.clone().map(|(path, _)| path));
    let java_home = java_from_home
        .map(|(_, home)| home)
        .or_else(|| studio_java.map(|(_, home)| home))
        .or_else(|| {
            java_path.as_ref().and_then(|path| {
                std::path::Path::new(path)
                    .parent()
                    .and_then(|bin| bin.parent())
                    .map(|home| home.display().to_string())
            })
        });

    let java_version = java_path.as_deref().and_then(|path| run_version(path, &["-version"]));
    let java = java_path.is_some();
    if !java {
        missing.push("Java (JDK 17)".to_string());
    }

    let kotlinc_path = which_tool("kotlinc").or_else(|| which_tool("kotlinc.bat"));
    let kotlinc_version = kotlinc_path
        .as_deref()
        .and_then(|path| run_version(path, &["-version"]));
    let kotlinc = kotlinc_path.is_some();

    // Gradle del PATH o una distribución descargada por el wrapper.
    let gradle_path = which_tool("gradle").or_else(|| {
        let home = home_dir()?;
        let wrapper = home.join(".gradle").join("wrapper").join("dists");
        find_file_under(
            &wrapper,
            if cfg!(windows) { "gradle.bat" } else { "gradle" },
            6,
        )
        .map(|path| path.display().to_string())
    });
    let gradle_version = gradle_path
        .as_deref()
        .and_then(|path| run_version(path, &["-version"]));
    let gradle = gradle_path.is_some();
    if !gradle {
        missing.push("Gradle".to_string());
    }

    // Android SDK.
    let android_sdk_path = std::env::var_os("ANDROID_HOME")
        .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(|base| std::path::PathBuf::from(base).join("Android").join("Sdk"))
        })
        .or_else(|| home_dir().map(|home| home.join("Android").join("Sdk")))
        .or_else(|| Some(std::path::PathBuf::from(
            "/Users/Shared/Android/sdk",
        )))
        .filter(|path| path.is_dir());
    let android_sdk = android_sdk_path.is_some();
    if !android_sdk {
        missing.push("Android SDK".to_string());
    }

    if !kotlinc {
        notes.push(
            "Kotlin no es imprescindible: Gradle descarga el compilador si hace falta."
                .to_string(),
        );
    }
    if !android_sdk {
        notes.push(
            "Instala Android Studio (incluye SDK, JDK y Gradle) o exporta ANDROID_HOME."
                .to_string(),
        );
    }

    // Los proyectos generados incluyen el wrapper, así que no hace falta un
    // Gradle instalado en el sistema: el wrapper descarga la versión fijada.
    let can_build = java && android_sdk;
    if !gradle {
        notes.push(
            "No hay Gradle en el PATH, pero el proyecto incluye el wrapper: se descargará automáticamente."
                .to_string(),
        );
    }

    BuildToolchain {
        os: std::env::consts::OS.to_string(),
        java,
        java_path,
        java_version,
        java_home,
        kotlinc,
        kotlinc_version,
        gradle,
        gradle_path,
        gradle_version,
        android_sdk,
        android_sdk_path: android_sdk_path.map(|path| path.display().to_string()),
        can_build,
        missing,
        notes,
    }
}
