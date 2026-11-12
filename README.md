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
- **Soporte Zilla HLS & Video Directo**: Inyeccion automatica de cabeceras seguras (`Sec-Fetch-Mode`, `Origin`, `Referer`, `User-Agent`) para prevenir bloqueos HTTP 403 Forbidden.
- **Separacion SUB / DUB**: Clasificacion automatica de audio con selector de idioma preferido y ordenamiento en `sortVideos`.

### 2. Firma Inmutable y Determinista (Sin Errores de Actualizacion)
- Cada extension generada incorpora de forma estatica el certificado `signingkey.jks` con alias `unbox` y huella SHA-256 inmutable:
  ```text
  8c36c461d49b9a49053fc14f67785e1bd91f79eb1b6ff382d384eb9735266de3
  ```
- **Compatibilidad Garantizada**: Elimina el fallo de Android "Firma diferente al actualizar" permitiendo instalar nuevas versiones de tus extensiones sobre versiones previas sin desinstalar.

### 3. Doble Flujo de Creacion
- **Modo Local**: Compila y empaqueta el APK directo en el almacenamiento de tu dispositivo o PC sin depender de servicios externos.
- **Modo Repositorio GitHub**:
  - Crea o actualiza automaticamente tu repositorio en GitHub mediante API.
  - Sube la estructura Gradle completa, el codigo fuente Kotlin, el icono mipmap y el archivo de firma.
  - Despliega un workflow de GitHub Actions que compila los APKs y publica a la rama `gh-pages` con su catalogo `index.min.json` y `repo.json`.
  - Genera un archivo `README.md` con enlaces directos tipo `nekoyomi://add-repo?url=...` para anadir el repositorio a Nekoyomi en un solo toque.

### 4. Autenticacion GitHub Adaptativa
- **En PC (Desktop)**: Deteccion automatica del inicio de sesion de **GitHub CLI (`gh auth token`)** o uso de Personal Access Token (PAT).
- **En Android (Movil)**: Panel superior de sesion con soporte de Personal Access Token (PAT) con permisos de `repo` y `workflow`, visualizacion de usuario, cambio rapido de cuenta y cierre seguro.

### 5. Servidor API y Soporte MCP para Inteligencia Artificial (Android & PC)
- Dado que en Android no se dispone de una terminal interactiva estandar, AEXT ejecuta un servidor HTTP nativo en Rust en el puerto local `:45731`:
  - `GET /api/status`: Diagnostico del sistema, plataforma, memoria y credenciales.
  - `GET /api/extensions`: Listado de extensiones creadas y enlaces de instalacion.
  - `GET /api/download/:id`: Descarga directa de APK compilado al movil.
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
