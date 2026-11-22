/// Plantilla del build.gradle.kts del módulo.

macro_rules! build_gradle_kt_fmt {
    () => { r#"plugins {{
    id("com.android.application")
    kotlin("plugin.serialization")
}}

android {{
    namespace = "{pkg_id}"
    compileSdk = 34

    defaultConfig {{
        applicationId = "{pkg_id}"
        minSdk = 21
        targetSdk = 34
        versionCode = {version_code}
        versionName = "{version_name}"
    }}

    base {{
        archivesName.set("aniyomi-{pkg_lang}.{pkg_slug}-v{version_name}")
    }}

    signingConfigs {{
        create("release") {{
            storeFile = rootProject.file("signingkey.jks")
            storePassword = "aextsigning"
            keyAlias = "aext"
            keyPassword = "aextsigning"
        }}
    }}

    buildTypes {{
        release {{
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
        }}
    }}

    // El lint de release intenta resolver dependencias transitivas muertas del
    // extensions-lib antiguo (injekt-core de JitPack) y rompe la compilación.
    // Las extensiones no necesitan lint vital; la compilación Kotlin basta.
    lint {{
        checkReleaseBuilds = false
    }}

    compileOptions {{
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }}

    kotlin {{
        compilerOptions {{
            jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_1_8
            freeCompilerArgs.add("-opt-in=kotlinx.serialization.ExperimentalSerializationApi")
        }}
    }}
}}

dependencies {{
    compileOnly("{extensions_lib}")
    compileOnly("androidx.preference:preference:1.2.1")
    compileOnly("uy.kohesive.injekt:injekt-core:1.16.1")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.2")
    implementation("org.jsoup:jsoup:1.17.2")
    implementation("com.squareup.okhttp3:okhttp:5.0.0-alpha.12")
}}
"# };
}
pub(crate) use build_gradle_kt_fmt;

/// Plantilla del settings.gradle.kts.
macro_rules! settings_gradle_kt_fmt {
    () => { r#"pluginManagement {{
    repositories {{
        gradlePluginPortal()
        google()
        mavenCentral()
    }}
}}

dependencyResolutionManagement {{
    repositoriesMode.set(RepositoriesMode.PREFER_SETTINGS)
    repositories {{
        google()
        mavenCentral()
        maven(url = "https://jitpack.io")
    }}
}}

rootProject.name = "nekoyomi_extensions"

include(":src:{pkg_lang}:{pkg_slug}")
"# };
}
pub(crate) use settings_gradle_kt_fmt;

/// Build raíz: AGP 9 lleva Kotlin integrado.
pub const ROOT_BUILD_GRADLE: &str = r#"plugins {
    id("com.android.application") version "9.2.1" apply false
    id("org.jetbrains.kotlin.plugin.serialization") version "2.3.21" apply false
}

tasks.register("clean", Delete::class) {
    delete(rootProject.layout.buildDirectory)
}
"#;

pub const GRADLE_PROPERTIES: &str = r#"org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
android.useAndroidX=true
android.enableJetifier=true
kotlin.code.style=official
"#;

pub const GITIGNORE: &str = r#".gradle/
build/
**/build/
.idea/
*.iml
local.properties
__pycache__/
*.pyc
"#;
