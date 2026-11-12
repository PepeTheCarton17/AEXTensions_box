# 📦 AEXT - Automatizador y Gestor de Extensiones Nekoyomi (Desktop & Mobile)

**AEXT** es una aplicación multiplataforma de alto rendimiento desarrollada en **Rust** con **Tauri v2**, diseñada para convertir cualquier sitio web de anime o manga en una extensión completa para **Nekoyomi** y **Aniyomi** de forma 100% determinista y **sin necesidad de IA**.

---

## 🚀 Características Principales

1. **Generación 100% Determinista (Sin IA)**:
   - Motor de análisis web de alta velocidad en Rust (`scraper`, `reqwest`).
   - Reconocimiento automático de motores CMS: **SvelteKit**, **DooPlay**, **Madara**, **MangaThemesia**, o genérico.
   - Detección de reproductores de video (Zilla HLS, Voe, MP4Upload, StreamWish, etc.) con inyección de cabeceras seguras (`Sec-Fetch-Mode`, `Origin`, `Referer`) para evitar errores HTTP 403.
   - Separación inteligente de pistas de doblaje (`[SUB]` y `[DUB]`) con configuración de usuario en `setupPreferenceScreen` y ordenamiento dinámico en `sortVideos`.

2. **Doble Modo de Creación (Pregunta Previa)**:
   - 📦 **Modo Local**: Genera el código Kotlin, manifiesto con `<uses-feature android:name="tachiyomi.animeextension" />`, gradle y paquete listo para instalar directo en tu dispositivo o probar en tu entorno sin depender de GitHub.
   - 🚀 **Modo Repositorio GitHub**: Se conecta a la API de GitHub, crea o actualiza tu repositorio, sube el código fuente, la firma determinista `signingkey.jks`, y configura un workflow de GitHub Actions que compila el APK y publica automáticamente a la rama `gh-pages` con su `repo.json` e `index.min.json`.

3. **Autenticación GitHub Flexible (Desktop & Mobile)**:
   - **En PC (Desktop)**: Detección automática de sesión existente mediante **GitHub CLI (`gh auth token`)** o mediante Personal Access Token (PAT).
   - **En Celular (Android Mobile)**: Ventana/modal superior donde ingresar tu Personal Access Token (PAT) con scopes `repo` y `workflow`, con botones para cerrar sesión, cambiar de cuenta o consultar scopes.

4. **Servidor API Embebido para Móvil (`:45731`)**:
   - Dado que en Android no existe una terminal bash/cmd estándar, AEXT levanta un servidor HTTP local nativo en Rust con **Axum**:
     - `GET /api/status`: Estado del sistema, memoria, plataforma y sesión.
     - `GET /api/extensions`: Catálogo de extensiones creadas en formato compatible con Nekoyomi.
     - `GET /api/download/:id`: Descarga directa del APK generado.
     - `GET /api/logs`: Terminal de diagnóstico y registros en tiempo real.

5. **Panel de Gestión de Extensiones**:
   - Catálogo interactivo de todas tus extensiones creadas con iconos generados automáticamente en resoluciones mipmap (`hdpi`, `xhdpi`, `xxhdpi`, `xxxhdpi`).
   - Botón directo **Instalar / Descargar APK**.
   - Botón **Copiar enlace de Repositorio** para añadirlo a Nekoyomi en un solo toque (`https://.../gh-pages/repo.json`).
   - Botón directo para inspeccionar el repositorio en GitHub.

6. **Diagnóstico & Solución de Errores Temporales**:
   - Consola de logs en vivo integrada.
   - Herramienta para probar extractores y verificar si los servidores de streaming devuelven 403 Forbidden o autorizan la conexión con las cabeceras requeridas.

---

## 🛠️ Tecnologías Empleadas

- **Backend**: Rust 1.77+, Tauri v2, Tokio, Axum, Scraper, Reqwest, Image (Lanczos3), Serde.
- **Frontend**: HTML5, Vite 6, Tailwind CSS, JavaScript ES Modules, Lucide Icons.
- **Android / Nekoyomi Engine**: Kotlin, OkHttp3, Jsoup, AndroidX Preference, GitHub Actions.

---

## 💻 Ejecución y Desarrollo

### Requisitos
- [Node.js](https://nodejs.org/) (v18 o superior)
- [Rust & Cargo](https://rustup.rs/)
- Android SDK & NDK (solo si deseas compilar el APK de AEXT para Android)

### Instalar dependencias
```bash
npm install
```

### Ejecutar en modo desarrollo (Desktop)
```bash
npm run tauri dev
```

### Compilar binario de escritorio (Windows / Linux / macOS)
```bash
npm run tauri build
```

### Compilar APK para Android
```bash
npm run tauri android init
npm run tauri android build
```

---

## 📄 Licencia
Proyecto desarrollado para el ecosistema Nekoyomi / Aniyomi por **PepeTheCarton**.
