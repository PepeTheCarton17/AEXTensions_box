# AEXT - Automatizador y Gestor de Extensiones Nekoyomi (Desktop & Mobile)

AEXT es una aplicacion multiplataforma de alto rendimiento desarrollada en Rust con Tauri v2, disenada para convertir cualquier sitio web de anime o manga en una extension completa y nativa para Nekoyomi y Aniyomi de forma 100% determinista y sin necesidad obligatoria de IA.

---

## Descargas Oficiales (Releases)

Los binarios precompilados para todas las plataformas estan disponibles directamente en la seccion de [GitHub Releases](https://github.com/PepeTheCarton17/AEXTensions_box/releases/latest):

| Plataforma | Paquete / Instalador | Arquitectura | Enlace de Descarga |
| :--- | :--- | :--- | :--- |
| **Android (Movil)** | `AEXT-v1.0.0-android-universal.apk` | Universal (arm64-v8a, armeabi-v7a, x86_64) | [Descargar APK](https://github.com/PepeTheCarton17/AEXTensions_box/releases/latest) |
| **Android (Movil arm64)** | `AEXT-v1.0.0-android-arm64-v8a.apk` | arm64-v8a nativo | [Descargar APK](https://github.com/PepeTheCarton17/AEXTensions_box/releases/latest) |
| **Windows (PC Instalador)** | `AEXT-v1.0.0-windows-x64-setup.exe` | x86_64 (Windows 10/11) | [Descargar Setup](https://github.com/PepeTheCarton17/AEXTensions_box/releases/latest) |
| **Windows (PC MSI)** | `AEXT-v1.0.0-windows-x64.msi` | x86_64 (Instalador Corporativo) | [Descargar MSI](https://github.com/PepeTheCarton17/AEXTensions_box/releases/latest) |
| **Linux (PC Portable)** | `AEXT-v1.0.0-linux-x86_64.AppImage` | x86_64 (Cualquier distribucion) | [Descargar AppImage](https://github.com/PepeTheCarton17/AEXTensions_box/releases/latest) |
| **Linux (PC Debian/Ubuntu)** | `AEXT-v1.0.0-linux-amd64.deb` | x86_64 (apt/dpkg) | [Descargar DEB](https://github.com/PepeTheCarton17/AEXTensions_box/releases/latest) |

---

## Caracteristicas Principales

### 1. Generacion Determinista de Extensiones
- **Motor de Analisis Web en Rust**: Deteccion automatica de selectores DOM (`scraper`, `reqwest`), esquemas CMS (SvelteKit, DooPlay, Madara, MangaThemesia, etc.) y endpoints de APIs.
- **Filtros e Idioma Predeterminado (Espanol)**: Soporte multilenguaje con espanol (`es`) por defecto para nombres, preferencias y listas de generos.
- **Lista Negra de Generos (#) y Servidores**:
  - Filtro interactivo `BlacklistGenresFilter` en el catalogo para excluir etiquetas no deseadas (Aventura, Ecchi, Gore, etc.).
  - Preferencia persistente `server_blacklist` en los ajustes de la extension para ignorar reproductores lentos o inestables.
- **Motor de Extractores Real (no solo iframes)**: La extension generada resuelve el embed de cada servidor antes de entregar el video:
  - `MP4Upload`, `Voe`, `Byse / StreamWish`, `Zilla / HLS` y un extractor generico para hosts desconocidos.
  - Sigue redirecciones por JavaScript (`window.location.href = "..."`), habituales en hosts que rotan de dominio.
  - Detecta todas las calidades reales del servidor (`label`, `quality`, `res`, `RESOLUTION=`) y **nunca inventa** una calidad: si no puede determinarla usa `Auto` (tambien evita leer `300px` del CSS como si fuera `300p`).
  - Expande el **master HLS** (`#EXT-X-STREAM-INF`) en una entrada `Video` por variante, con su resolucion y bitrate (`1080p (4500 kbps)`, `720p (900 kbps)`, ...). El reproductor muestra asi todas las calidades disponibles de la misma fuente.
  - Inyecta cabeceras de Cloudflare en playlist, variantes y segmentos (`User-Agent`, `Referer`, `Origin`, `Accept`, `Sec-Fetch-Dest/Mode/Site`), evitando el `403 Forbidden` y el salto al final del video por segmentos rechazados.
  - Usa `network.cloudflareClient` para heredar cookies de Cloudflare y hace un warm-up de la pagina del player para que el reto se resuelva antes de pedir la playlist.
  - Nota honesta: los hosts que hoy sirven el video solo desde una SPA/JS ofuscado (por ejemplo `Byse`) pueden necesitar una actualizacion especifica de su extractor. AEXT no devuelve videos de prueba ni calidades inventadas: si no puede resolverlos, simplemente no los lista.
- **Separacion SUB / DUB**: Deteccion automatica por bloque del HTML (`SUB`, `DUB`, `LAT`, `CAST`), con prefijo `[SUB]`/`[DUB]` y selector de audio preferido.
- **Preferencias del reproductor**: servidor preferido, calidad preferida y lista negra de servidores; el orden final es audio -> servidor -> resolucion descendente.

### 2. Firma Inmutable y Determinista (Sin Errores de Actualizacion)
- Cada extension generada incorpora de forma estatica el certificado `signingkey.jks` con alias `unbox` y huella SHA-256 inmutable:
  ```text
  8c36c461d49b9a49053fc14f67785e1bd91f79eb1b6ff382d384eb9735266de3
  ```
- **Compatibilidad Garantizada**: Elimina el fallo de Android "Firma diferente al actualizar" permitiendo instalar nuevas versiones de tus extensiones sobre versiones previas sin desinstalar.

### 3. Doble Flujo de Creacion
- **Modo Local (codigo fuente)**:
  - Escribe la extension completa en `%APPDATA%/AEXT/extensions/<pkg_id>` (o `~/.aext`).
  - El boton de descarga entrega un **ZIP real del codigo fuente** (`application/zip`), listo para abrir en Android Studio o subir a tu repo.
  - No compila el APK en el dispositivo: para obtener el APK hay que compilar el proyecto (Gradle local o GitHub Actions).
- **Modo Repositorio GitHub**:
  - Reutiliza un repositorio existente de tu cuenta (selector de repos + rama por defecto real) en lugar de crear siempre `owner/repo` en `main`. Si el repo ya tiene extensiones, sus modulos se conservan y se fusionan en `settings.gradle.kts`.
  - Sube la estructura Gradle multi-modulo identica a `repo_unbox`:
    ```text
    settings.gradle.kts
    build.gradle.kts
    gradle.properties
    gradle/wrapper/...
    .github/workflows/build_push.yml
    .github/scripts/generate-repo-index.py
    src/<lang>/<slug>/build.gradle.kts
    src/<lang>/<slug>/src/main/AndroidManifest.xml
    src/<lang>/<slug>/src/main/java/.../<Clase>.kt
    ```
  - El workflow compila con `gradle assembleRelease` y publica APK, icono, `index.min.json` y `repo.json` en la rama `gh-pages`.
  - El boton **Ver compilacion** abre GitHub Actions (no promete una URL de APK inexistente).
  - Genera un `README.md` con enlaces `nekoyomi://add-repo?url=...` para agregar el repositorio en un toque.
  - El indice se genera a partir de los `applicationId`, `versionCode`, `versionName` y `override val name` reales del codigo.

### 4. Autenticacion GitHub Adaptativa
- **En PC (Desktop)**: Deteccion automatica del inicio de sesion de **GitHub CLI (`gh auth token`)** o uso de Personal Access Token (PAT).
- **En Android (Movil)**: Panel superior de sesion con soporte de Personal Access Token (PAT) con permisos de `repo` y `workflow`, visualizacion de usuario, cambio rapido de cuenta y cierre seguro.

### 5. Servidor API y Soporte MCP para Inteligencia Artificial (Android & PC)
- Dado que en Android no se dispone de una terminal interactiva estandar, AEXT ejecuta un servidor HTTP nativo en Rust en el puerto local `:45731`:
  - `GET /api/status`: Diagnostico del sistema, plataforma, memoria y credenciales.
  - `GET /api/extensions`: Listado de extensiones creadas y enlaces de instalacion.
  - `GET /api/download/:id`: Descarga el **ZIP del codigo fuente** de la extension (modo local) con `Content-Disposition`; en modo GitHub apunta a la compilacion.
  - `GET /api/logs`: Flujo de registros y logs en tiempo real para diagnosticar scrapers o fallos de compilacion.
  - **Soporte MCP (Model Context Protocol)**: Permite que asistentes de IA (en PC o a traves de la red local) inspeccionen el proceso, asistan en la creacion de filtros complejos y verifiquen el estado de los repositorios.

### 6. Interfaz Limpia y Profesional (Cero Emojis)
- Diseno estilizado con iconos vectoriales SVG limpios y uniformes.
- Consola de logs integrada y herramientas de diagnostico de servidores.

---

## Aviso Legal / Disclaimer

> **ADVERTENCIA LEGAL IMPORTANTE**:
>
> 1. **Herramienta Educativa y de Investigacion**: AEXT es un software de codigo abierto desarrollado exclusivamente con fines tecnicos, educativos y de investigacion en desarrollo de software y analisis web.
> 2. **Sin Alojamiento de Contenido**: Ni AEXT ni sus desarrolladores alojan, gestionan, indexan, transmiten ni distribuyen ningun tipo de contenido multimedia, archivos de video, audio, imagenes o capitulos de manga/anime protegidos por derechos de autor.
> 3. **Operacion de Terceros**: Las extensiones creadas por este software actuan unicamente como analizadores sintacticos (scrapers) que procesan informacion publica disponible en sitios web de terceros independientes y no afiliados.
> 4. **Derechos de Propiedad Intelectual**: Todas las marcas comerciales, nombres de series, logotipos y derechos de propiedad intelectual pertenecen exclusivamente a sus respectivos duenos y titulares.
> 5. **Responsabilidad del Usuario**: El usuario final asume total y exclusiva responsabilidad por el uso que haga de esta aplicacion y de las extensiones generadas, debiendo respetar los terminos de servicio de las plataformas analizadas y la legislacion vigente de propiedad intelectual en su jurisdiccion.

---

## Compilacion desde Codigo Fuente

### Requisitos
- Node.js (v20 o superior)
- Rust & Cargo (1.77 o superior)
- Android SDK & NDK r26+ (unicamente para empaquetar el APK de Android)

### Pasos
```bash
# 1. Clonar el repositorio
git clone https://github.com/PepeTheCarton17/AEXTensions_box.git
cd AEXTensions_box

# 2. Instalar dependencias del frontend
npm install

# 3. Compilar version de escritorio (Windows / Linux)
npm run tauri build

# 4. Compilar version de Android
npm run tauri android init
npm run tauri android build --apk
```

---

## Licencia
Distribuido bajo licencia MIT. Desarrollado para el ecosistema Nekoyomi / Aniyomi por PepeTheCarton.
