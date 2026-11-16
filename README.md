# AEXT - Automatizador y Gestor de Extensiones Nekoyomi/Aniyomi (Escritorio y Movil)

AEXT es una aplicacion de escritorio y movil construida con Tauri v2 (Rust + JavaScript) que analiza un sitio de anime o manga y genera una extension completa para Nekoyomi/Aniyomi. La generacion es **determinista**: las decisiones salen del HTML y del CMS detectado, no de un servicio externo. La app no aloja contenido ni incluye videos de prueba.

---

## Descargas oficiales

Cada push a `main` publica una release `v1.0.<numero_de_ejecucion>` en [GitHub Releases](https://github.com/PepeTheCarton17/AEXTensions_box/releases/latest):

| Plataforma | Paquete | Arquitectura |
| :--- | :--- | :--- |
| Windows | `AEXT-v1.0.<n>-windows-x64-setup.exe` | x86_64 |
| Windows | `AEXT-v1.0.<n>-windows-x64.msi` | x86_64 |
| Android | `AEXT-v1.0.<n>-android-universal.apk` | arm64-v8a, armeabi-v7a, x86_64 |
| Android | `AEXT-v1.0.<n>-android-arm64-v8a.apk` | arm64-v8a |
| Linux | `AEXT-v1.0.<n>-linux-x86_64.AppImage` / `.deb` | x86_64 |

La pestana **Ajustes & Descargas** muestra la tabla de assets reales de la ultima release (nombre, tamano y boton de descarga). Los enlaces externos se abren en el navegador del sistema, no dentro del webview.

> El numero `<n>` es el `run_number` de GitHub Actions. Por eso dos releases consecutivas nunca comparten version.

---

## Que hace

### 1. Analisis y generacion determinista
- Acepta una o varias URLs (una por linea). Detecta CMS, listado, detalle, paginacion, tipos de contenido (anime/manga) e idioma.
- El nombre y el paquete se derivan del **dominio**; los titulos genericos ("Watch", "Home", "Anime") se descartan para no producir nombres inestables.
- Genera un proyecto Gradle multi-modulo:
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
- Incluye `index.min.json`, `repo.json` y un `README.md` con enlaces `nekoyomi://add-repo?url=...`.
- En modo GitHub, el workflow compila con Gradle y publica los APK en la rama `gh-pages`. El boton **Ver compilacion** abre GitHub Actions; no promete una URL de APK que todavia no existe.

### 2. Motor de extractores real
- Resuelve embeds de `MP4Upload`, `Voe`, `Zilla/HLS`, `StreamWish/Byse` y hosts desconocidos.
- Sigue redirecciones por JavaScript (`window.location.href = "..."`) y busca `iframe`, `<video>`, `<source>`, atributos `data-*`, pares JSON y `.m3u8` en crudo.
- Detecta calidades reales (`label`, `quality`, `res`, `RESOLUTION=`) y **no inventa** calidades: si no puede determinarla usa `Auto` (evita leer `300px` de CSS como si fuera `300p`).
- Expande el **master HLS** (`#EXT-X-STREAM-INF`) en una entrada por variante, con resolucion y bitrate (`1080p (4500 kbps)`, `720p (900 kbps)`, ...).
- Inyecta cabeceras para HLS y segmentos (`User-Agent`, `Referer`, `Origin`, `Accept`, `Sec-Fetch-*`), usa `network.cloudflareClient` y hace warm-up de la pagina del player.
- Separacion **SUB/DUB** por bloque del HTML (`SUB`, `DUB`, `LAT`, `CAST`), con selector de audio preferido.
- Preferencias del reproductor: servidor preferido, calidad preferida y lista negra de servidores. El orden final es audio -> servidor -> resolucion descendente.

### 3. Firma inmutable
Cada extension generada incorpora el keystore `signingkey.jks` (alias `unbox`) con huella SHA-256 inmutable:

```text
8c36c461d49b9a49053fc14f67785e1bd91f79eb1b6ff382d384eb9735266de3
```

Asi las actualizaciones se instalan encima de la version anterior sin el error "Firma diferente al actualizar".

### 4. Doble flujo de creacion
- **Modo local (codigo fuente)**: escribe la extension en `%APPDATA%/AEXT/extensions/<pkg_id>` (o el directorio de datos de la app) y permite descargar un **ZIP real del codigo fuente** (`application/zip`). Tambien puedes abrir la carpeta del codigo desde la app.
- **Modo GitHub**: reutiliza un repositorio existente (selector + rama por defecto real) o crea uno nuevo. Si el repo ya tiene extensiones, conserva sus modulos y los fusiona en `settings.gradle.kts`.
- En ambos modos la extension se puede actualizar despues desde **Mis Extensiones** (sube `versionCode + 1` y `version = "16.<code>"`).

### 5. Compilacion local honesta
- La app comprueba si existen **Java, Gradle y el Android SDK**. Solo entonces habilita la compilacion local.
- Si falta algo, el boton explica que falta y enlaza al flujo correcto (Android Studio o GitHub Actions).
- El ZIP del codigo fuente **no es un APK**. AEXT no afirma compilar APK en el dispositivo: en Android se usa GitHub Actions o Android Studio en un PC.
- La compilacion escribe `local.properties` con `sdk.dir`, ejecuta `:<modulo>:assembleRelease` y muestra la salida de Gradle en la propia app.

### 6. Copias de seguridad y reversibilidad
- Antes de aplicar cualquier parche se guarda una copia del codigo en:
  ```text
  <extension>/.aext-backups/<AAAAMMDD-HHMMSS>/
    manifest.json
    ...copia del arbol de fuentes...
  ```
- La pestana **Herramientas & MCP** permite listar y restaurar copias. Restaurar **solo sobrescribe** archivos: nunca borra el resto del proyecto.
- Al restaurar tambien se sincronizan `versionCode`/`versionName` de la ficha de la extension con el `build.gradle.kts` recuperado, para que la UI no muestre una version que ya no existe en el codigo.
- Se excluyen de la copia `build/`, `.gradle/`, `.git/`, `.idea/` y la propia carpeta de backups.

### 7. Parche manual y MCP
- El endpoint `POST /api/ai/patch` aplica un **parche manual** a una extension local. No es un generador autonomo: solo toca lo que se le pide.
- Campos que puede cambiar:
  - `versionCode` y `versionName` (siempre sube la version).
  - Selectores de listado: `popularAnimeSelector`, `latestUpdatesSelector`, `searchAnimeSelector`.
  - Selectores de paginacion: `popularAnimeNextPageSelector`, `latestUpdatesNextPageSelector`, `searchAnimeNextPageSelector`.
  - Selector de titulo (`title = element.selectFirst("...")`).
  - Bloque de codigo Kotlin adicional para el reproductor y cabeceras HTTP personalizadas.
- Cada parche crea una copia previa (ver seccion 6), por lo que se puede volver al estado **original** o a una version **original modificable** anterior.
- El servidor MCP (`POST /mcp` y `GET /mcp/manifest`) expone el estado y las mismas operaciones a clientes MCP externos que se ejecuten **en la misma maquina**.

### 8. Interfaz e idioma
- Interfaz en **espanol e ingles**, conmutables en **Ajustes** y persistidas en `localStorage` (`aext_ui_lang`).
- Iconos SVG, sin emojis. Logo propio (circulo morado con "A").
- Consola de logs integrada, diagnosticos de servidores y panel de versiones/descargas.

### 9. API local y seguridad
El servidor HTTP nativo escucha **solo en loopback** (`127.0.0.1:45731`):

| Endpoint | Uso |
| :--- | :--- |
| `GET /api/status` | Estado, plataforma y credenciales |
| `GET /api/extensions` | Extensiones creadas y enlaces |
| `GET /api/download/:id` | ZIP del codigo fuente (modo local) |
| `GET /api/logs` | Logs en tiempo real |
| `POST /api/ai/patch` | Parche manual de una extension |
| `POST /mcp` | JSON-RPC de Model Context Protocol |
| `GET /mcp/manifest` | Descubrimiento de herramientas MCP |

No se abre a la red local. Las descargas de ZIP desde la app nativa las escribe Rust en la carpeta de Descargas y devuelven la ruta real del archivo.

---

## Versionado

GitHub Actions escribe la version `1.0.<run_number>` en:

- `package.json`
- `package-lock.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

Antes de compilar, para que la version que muestra la app (esquina superior izquierda, Ajustes y actualizador) coincida exactamente con la release publicada. Una compilacion local sin ese paso muestra la version de `Cargo.toml` (por ejemplo `1.0.0`).

---

## Compilacion desde codigo fuente

### Requisitos
- Node.js 20+
- Rust y Cargo 1.77+
- Dependencias de Tauri v2 para tu sistema
- Android SDK y NDK r26+ (solo para empaquetar el APK de Android)

### Pasos
```bash
git clone https://github.com/PepeTheCarton17/AEXTensions_box.git
cd AEXTensions_box

npm install
npm run tauri build

# Android
npm run tauri android init
npm run tauri android build --apk
```

### Compilar una extension generada (local)
1. Instala Android Studio (incluye JDK 17, Gradle y el Android SDK).
2. Configura `ANDROID_HOME` o deja que AEXT detecte el SDK.
3. Genera o abre la extension y pulsa **Compilar APK en local** en el modal de exito.
4. El APK queda en `src/<lang>/<slug>/build/outputs/apk/release/`.

---

## Limitaciones honestas

- Los hosts que sirven el video solo desde una SPA o JavaScript ofuscado (por ejemplo `Byse`) pueden requerir una actualizacion especifica de su extractor. Si AEXT no puede resolverlos, no los lista.
- Cloudflare puede bloquear segmentos HLS desde una IP concreta; las cabeceras y cookies reducen el problema, pero no lo eliminan en todos los casos.
- La compilacion local necesita el toolchain completo. En Android no se compila un APK en el dispositivo.
- El modo GitHub necesita un token con permisos `repo` y `workflow`.
- Restaurar una copia devuelve el codigo y la version de esa copia, pero no desinstala APKs ya instalados.

---

## Aviso legal

> **ADVERTENCIA LEGAL IMPORTANTE**:
>
> 1. **Herramienta educativa y de investigacion**: AEXT es software de codigo abierto desarrollado con fines tecnicos, educativos y de investigacion en desarrollo de software y analisis web.
> 2. **Sin alojamiento de contenido**: ni AEXT ni sus desarrolladores alojan, gestionan, indexan, transmiten ni distribuyen contenido multimedia, video, audio, imagenes o capitulos protegidos por derechos de autor.
> 3. **Operacion de terceros**: las extensiones generadas actuan unicamente como analizadores sintacticos (scrapers) de informacion publica disponible en sitios web de terceros independientes y no afiliados.
> 4. **Propiedad intelectual**: todas las marcas, nombres de series, logotipos y derechos pertenecen a sus respectivos titulares.
> 5. **Responsabilidad del usuario**: el usuario asume total responsabilidad por el uso de la aplicacion y de las extensiones generadas, respetando los terminos de servicio de las plataformas analizadas y la legislacion vigente.

---

## Licencia

Distribuido bajo licencia MIT. Desarrollado para el ecosistema Nekoyomi/Aniyomi por PepeTheCarton.
