// Puente Tauri v2 con fallback HTTP al servidor embebido (:45731).
let tauriCore = null;

async function initTauri() {
  if (window.__TAURI_INTERNALS__ || window.__TAURI__) {
    try {
      tauriCore = await import('@tauri-apps/api/core');
      console.log('[AEXT] Conectado exitosamente con Tauri v2 IPC.');
    } catch (e) {
      console.log('[AEXT] Modo web/browser sin runtime directo de Tauri. Usando fallback HTTP.');
    }
  } else {
    console.log('[AEXT] Entorno sin Tauri nativo. Usando fallback HTTP hacia :45731.');
  }
}

// Fallback para cuando corre en navegador web o móvil conectándose a la API embebida
async function invokeBackend(command, args = {}) {
  if (!tauriCore) {
    await initTauri();
  }
  if (tauriCore && tauriCore.invoke) {
    return await tauriCore.invoke(command, args);
  }

  // Fallback HTTP hacia el servidor embebido
  const API_BASE = 'http://localhost:45731/api';
  switch (command) {
    case 'get_gh_auth_status': {
      const res = await fetch(`${API_BASE}/status`);
      const data = await res.json();
      if (data.github_logged_in) {
        return { username: data.github_user, avatar_url: '', auth_type: 'pat', scopes: [] };
      }
      return null;
    }
    case 'analyze_url': {
      const res = await fetch(`${API_BASE}/analyze`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url: args.url })
      });
      return await res.json();
    }
    case 'get_created_extensions': {
      const res = await fetch(`${API_BASE}/extensions`);
      return await res.json();
    }
    case 'get_system_logs': {
      const res = await fetch(`${API_BASE}/logs`);
      return await res.json();
    }
    case 'check_aext_updates': {
      // Fallback de desarrollo: consulta la release real de GitHub.
      const res = await fetch(
        'https://api.github.com/repos/PepeTheCarton17/AEXTensions_box/releases/latest',
        { headers: { Accept: 'application/vnd.github+json' } }
      );
      if (!res.ok) throw new Error(`GitHub HTTP ${res.status}`);
      const release = await res.json();
      const tag = String(release.tag_name || '').replace(/^v/, '');
      return {
        current_version: 'dev',
        latest_version: tag,
        tag_name: release.tag_name || '',
        update_available: false,
        release_notes: release.body || '',
        release_url: release.html_url || '',
        assets: (release.assets || []).map((asset) => ({
          name: asset.name,
          url: asset.browser_download_url,
          size: asset.size,
          content_type: asset.content_type,
        })),
      };
    }
    case 'get_system_status': {
      const res = await fetch(`${API_BASE}/status`);
      const data = await res.json();
      return {
        app_version: data.version || '1.0.0',
        api_port: 45731,
        mcp_endpoint: data.mcp_endpoint || 'http://localhost:45731/mcp',
        is_mobile: data.platform === 'android',
        os: data.platform || 'unknown',
        active_extensions_count: data.extensions_count || 0,
        gh_logged_in: Boolean(data.github_logged_in),
        gh_user: data.github_user || null
      };
    }
    case 'list_github_repos': {
      return [];
    }
    case 'update_extension':
    case 'update_all_extensions':
    case 'update_extensions_batch':
    case 'sync_extension_catalog':
    case 'check_extensions_in_repo':
    case 'restore_extension_repo_state':
    case 'generate_extensions':
    case 'list_engines':
    case 'install_engine':
    case 'set_engine_enabled':
    case 'uninstall_engine':
    case 'test_engines':
      throw new Error('Esta acción requiere la aplicación nativa de AEXT (no está disponible en el modo navegador).');
    case 'delete_extension':
    case 'delete_extension_github':
    case 'delete_extension_permanently':
      throw new Error('Eliminar extensiones requiere la aplicación nativa de AEXT.');
    default:
      console.warn(`Comando '${command}' ejecutado en fallback HTTP`);
      return null;
  }
}

// Descarga un artefacto del servidor local.
// - En la app nativa Rust escribe el ZIP en Descargas y devuelve la ruta real
//   (WebView2 no siempre completa las descargas Blob ni avisa al usuario).
// - En navegador se usa un Blob para no navegar fuera de la app.
async function saveLocalZip(url, filename) {
  if (window.__TAURI_INTERNALS__ || window.__TAURI__) {
    const saved = await invokeBackend('save_extension_zip', { url, filename });
    if (!saved || !saved.path) {
      throw new Error('La descarga no devolvió una ruta de archivo.');
    }
    return saved;
  }

  const response = await fetch(url);
  if (!response.ok) {
    let detail = '';
    try {
      detail = await response.text();
    } catch (_) {
      detail = '';
    }
    try {
      const parsed = JSON.parse(detail);
      detail = parsed.error || detail;
    } catch (_) {
      // El cuerpo no era JSON; usamos el texto tal cual.
    }
    throw new Error(detail || `HTTP ${response.status}`);
  }

  const blob = await response.blob();
  const objectUrl = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = objectUrl;
  link.download = filename || 'extension.zip';
  document.body.appendChild(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(objectUrl), 5000);
  return { path: null, bytes: blob.size, filename: filename || 'extension.zip' };
}

// ==================== ENLACES EXTERNOS ====================
// El webview de Tauri no debe navegar fuera de la app: todo enlace http(s)
// se abre en el navegador del sistema mediante el comando nativo `open_url`.
async function openExternal(url) {
  if (!url) return false;
  try {
    await invokeBackend('open_url', { url });
    return true;
  } catch (error) {
    try {
      window.open(url, '_blank', 'noopener');
      return true;
    } catch (_) {
      return false;
    }
  }
}

function initExternalLinks() {
  document.addEventListener('click', (event) => {
    const link = event.target.closest('a[href^="http"]');
    if (link && !link.hasAttribute('download')) {
      event.preventDefault();
      openExternal(link.href);
      return;
    }
    const opener = event.target.closest('[data-open-url]');
    if (opener) {
      event.preventDefault();
      openExternal(opener.getAttribute('data-open-url'));
    }
  });
}

export {
  invokeBackend,
  openExternal,
  saveLocalZip,
  initExternalLinks,
};

