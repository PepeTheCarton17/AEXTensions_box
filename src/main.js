// AEXT Client Engine - Tauri v2 & Embedded API Bridge

import { initLanguage, setLanguage, getLanguage, t, applyLanguage } from './i18n.js';

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
    case 'generate_extensions':
      throw new Error('Esta acción requiere la aplicación nativa de AEXT (no está disponible en el modo navegador).');
    case 'delete_extension':
      throw new Error('Eliminar extensiones requiere la aplicación nativa de AEXT.');
    default:
      console.warn(`Comando '${command}' ejecutado en fallback HTTP`);
      return null;
  }
}

// ==================== APP STATE ====================
const state = {
  activeTab: 'tab-generator',
  currentUser: null,
  analyzedSites: [],
  extensions: [],
  logs: [],
  pendingGenerationMode: 'local',
  githubRepos: [],
  appVersion: '',
  lastRelease: null,
  successExt: null,
  selectedBackupExt: '',
  toolchain: null,
};

// ==================== DOM ELEMENTS ====================
const elements = {
  // Tabs
  tabBtns: document.querySelectorAll('.tab-btn'),
  tabPanes: document.querySelectorAll('.tab-pane'),

  // Header & Auth
  btnOpenAuthModal: document.getElementById('btn-open-auth-modal'),
  btnCloseAuthModal: document.getElementById('btn-close-auth-modal'),
  modalGithubAuth: document.getElementById('modal-github-auth'),
  authBtnText: document.getElementById('auth-btn-text'),
  headerAvatar: document.getElementById('header-avatar'),
  authLoggedInView: document.getElementById('auth-logged-in-view'),
  authLoggedOutView: document.getElementById('auth-logged-out-view'),
  modalUserAvatar: document.getElementById('modal-user-avatar'),
  modalUserName: document.getElementById('modal-user-name'),
  modalUserLogin: document.getElementById('modal-user-login'),
  modalAuthTypeBadge: document.getElementById('modal-auth-type-badge'),
  btnLoginCli: document.getElementById('btn-login-cli'),
  btnSubmitPat: document.getElementById('btn-submit-pat'),
  inputPatToken: document.getElementById('input-pat-token'),
  btnSwitchAccount: document.getElementById('btn-switch-account'),
  btnLogout: document.getElementById('btn-logout'),

  // Generator
  inputUrls: document.getElementById('input-urls'),
  btnAnalyzeUrls: document.getElementById('btn-analyze-urls'),
  btnStartGenFlow: document.getElementById('btn-start-generation-flow'),
  analysisPreviewContainer: document.getElementById('analysis-preview-container'),
  analysisCards: document.getElementById('analysis-cards'),
  terminalLogs: document.getElementById('terminal-logs'),
  btnClearLogs: document.getElementById('btn-clear-logs'),

  // Generation Mode Modal
  modalGenMode: document.getElementById('modal-generation-mode'),
  btnCancelGenMode: document.getElementById('btn-cancel-generation-mode'),
  btnConfirmGenMode: document.getElementById('btn-confirm-generation-mode'),
  githubRepoExtraFields: document.getElementById('github-repo-extra-fields'),
  inputCustomRepoName: document.getElementById('input-custom-repo-name'),
  selectExistingRepo: document.getElementById('select-existing-repo'),
  btnRefreshRepos: document.getElementById('btn-refresh-repos'),

  // Catalog
  extensionsCatalogGrid: document.getElementById('extensions-catalog-grid'),
  badgeExtCount: document.getElementById('badge-ext-count'),
  btnRefreshExtensions: document.getElementById('btn-refresh-extensions'),

  // Diagnostics & Stream Tester
  testStreamInput: document.getElementById('test-stream-input'),
  btnTestStream: document.getElementById('btn-test-stream'),
  testStreamResult: document.getElementById('test-stream-result'),

  // MCP diagnostics & local patches
  btnCopyMcpConfig: document.getElementById('btn-copy-mcp-config'),
  btnGenAiDiag: document.getElementById('btn-generate-ai-diagnostic'),
  btnCopyLogsAi: document.getElementById('btn-copy-logs-ai'),
  inputAiPatch: document.getElementById('input-ai-patch'),
  btnApplyAiPatch: document.getElementById('btn-apply-ai-patch'),

  // Settings
  btnCheckUpdates: document.getElementById('btn-check-updates'),
  btnSaveSettings: document.getElementById('btn-save-settings'),
  settingsLang: document.getElementById('settings-lang'),
  settingsAppUiLang: document.getElementById('settings-app-ui-lang'),
  settingsDefaultRepo: document.getElementById('settings-default-repo'),
  selectExtLang: document.getElementById('select-extension-lang'),
  checkPrivateRepo: document.getElementById('check-private-repo'),
  appVersionBadge: document.getElementById('app-version-badge'),

  // Generation Modal fields
  inputConfirmExtName: document.getElementById('input-confirm-ext-name'),
  selectConfirmExtType: document.getElementById('select-confirm-ext-type'),
  selectConfirmExtLang: document.getElementById('select-confirm-ext-lang'),

  // Tools: backups
  selectBackupExt: document.getElementById('select-backup-ext'),
  btnRefreshBackups: document.getElementById('btn-refresh-backups'),
  btnOpenSelectedFolder: document.getElementById('btn-open-selected-folder'),
  backupsList: document.getElementById('backups-list'),

  // Settings / releases
  settingsCurrentVersion: document.getElementById('settings-current-version'),
  releaseAssetsBody: document.getElementById('release-assets-body'),
  customRepoWrapper: document.getElementById('custom-repo-wrapper'),

  // Success Modal
  modalGenSuccess: document.getElementById('modal-generation-success'),
  successExtIcon: document.getElementById('success-ext-icon'),
  successExtName: document.getElementById('success-ext-name'),
  successExtPkg: document.getElementById('success-ext-pkg'),
  successExtType: document.getElementById('success-ext-type'),
  successExtVersion: document.getElementById('success-ext-version'),
  successDownloadStatus: document.getElementById('success-download-status'),
  successBuildSection: document.getElementById('success-build-section'),
  successToolchainStatus: document.getElementById('success-toolchain-status'),
  successCompileOutput: document.getElementById('success-compile-output'),
  btnSuccessOpenFolder: document.getElementById('btn-success-open-folder'),
  btnSuccessCompile: document.getElementById('btn-success-compile'),
  btnSuccessInstallApk: document.getElementById('btn-success-install-apk'),
  btnSuccessInstallLabel: document.getElementById('btn-success-install-label'),
  btnSuccessDeeplinkRepo: document.getElementById('btn-success-deeplink-repo'),
  btnSuccessCopyRepo: document.getElementById('btn-success-copy-repo'),
  btnCloseSuccessModal: document.getElementById('btn-close-success-modal'),
};

// ==================== LOGGING UTILITIES ====================
function appendTerminalLog(level, message) {
  const time = new Date().toLocaleTimeString();
  const line = document.createElement('div');
  line.className = 'flex items-start gap-2 text-[11px] leading-relaxed';

  let colorClass = 'text-slate-300';
  let badgeColor = 'text-slate-400 bg-surface-800';
  if (level === 'SUCCESS') {
    colorClass = 'text-emerald-300';
    badgeColor = 'text-emerald-400 bg-emerald-500/10 border border-emerald-500/20';
  } else if (level === 'ERROR') {
    colorClass = 'text-rose-300';
    badgeColor = 'text-rose-400 bg-rose-500/10 border border-rose-500/20';
  } else if (level === 'WARN') {
    colorClass = 'text-amber-300';
    badgeColor = 'text-amber-400 bg-amber-500/10 border border-amber-500/20';
  }

  line.innerHTML = `
    <span class="text-slate-600 select-none">${time}</span>
    <span class="px-1.5 py-0.2 rounded text-[9px] font-bold uppercase ${badgeColor}">${level}</span>
    <span class="${colorClass} flex-1 break-all">${escapeHtml(message)}</span>
  `;

  elements.terminalLogs.appendChild(line);
  elements.terminalLogs.scrollTop = elements.terminalLogs.scrollHeight;
}

function escapeHtml(str) {
  if (!str) return '';
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

// Icono de respaldo determinista: inicial del nombre sobre un color estable.
// Evita que las extensiones sin favicon se vean como imágenes rotas.
function fallbackIcon(name) {
  const label = (name || '?').trim();
  const letter = (label.charAt(0) || '?').toUpperCase();
  let hash = 0;
  for (let i = 0; i < label.length; i++) {
    hash = (hash * 31 + label.charCodeAt(i)) | 0;
  }
  const hue = Math.abs(hash) % 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96">` +
    `<rect width="96" height="96" rx="20" fill="hsl(${hue}, 62%, 42%)"/>` +
    `<text x="48" y="63" font-size="46" font-family="Arial,Helvetica,sans-serif" font-weight="700" fill="#ffffff" text-anchor="middle">${letter.replace(/[<>&"']/g, '')}</text>` +
    `</svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function iconFor(ext) {
  const icon = ext && typeof ext.icon_base64 === 'string' ? ext.icon_base64.trim() : '';
  return icon || fallbackIcon(ext && ext.name);
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

function formatBytes(bytes) {
  const value = Number(bytes) || 0;
  if (value <= 0) return '';
  const units = ['B', 'KB', 'MB', 'GB'];
  let size = value;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(size >= 10 || index === 0 ? 0 : 1)} ${units[index]}`;
}

function releaseAssetMeta(name) {
  const lower = String(name || '').toLowerCase();
  if (lower.endsWith('.apk')) {
    const arch = lower.includes('arm64') ? 'arm64-v8a' : 'universal';
    return { platform: 'Android (Mobile)', arch, tone: 'emerald', icon: 'android' };
  }
  if (lower.endsWith('.msi')) {
    return { platform: 'Windows (PC) MSI', arch: 'x86_64', tone: 'sky', icon: 'windows' };
  }
  if (lower.endsWith('setup.exe')) {
    return { platform: 'Windows (PC) Setup', arch: 'x86_64', tone: 'sky', icon: 'windows' };
  }
  if (lower.endsWith('.exe')) {
    return { platform: `Windows (PC) ${t('rel.portable')}`, arch: 'x86_64', tone: 'sky', icon: 'windows' };
  }
  if (lower.endsWith('.appimage')) {
    return { platform: 'Linux (PC) AppImage', arch: 'x86_64', tone: 'amber', icon: 'linux' };
  }
  if (lower.endsWith('.deb')) {
    return { platform: 'Linux (PC) Debian', arch: 'x86_64', tone: 'amber', icon: 'linux' };
  }
  return { platform: 'Archivo', arch: '—', tone: 'slate', icon: 'file' };
}

function releaseIcon(kind, tone) {
  if (kind === 'android') {
    return `<svg class="w-4 h-4 text-${tone}-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z"/></svg>`;
  }
  if (kind === 'windows') {
    return `<svg class="w-4 h-4 text-${tone}-400" fill="currentColor" viewBox="0 0 24 24"><path d="M0 3.449L9.75 2.1v9.451H0m10.949-9.602L24 0v11.4H10.949M0 12.6h9.75v9.451L0 20.699M10.949 12.6H24V24l-12.901-1.799"/></svg>`;
  }
  if (kind === 'linux') {
    return `<svg class="w-4 h-4 text-${tone}-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"/></svg>`;
  }
  return `<svg class="w-4 h-4 text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/></svg>`;
}

function renderReleaseTable(release) {
  if (!elements.releaseAssetsBody) return;
  state.lastRelease = release || state.lastRelease;
  const data = state.lastRelease;
  const assets = (data && Array.isArray(data.assets)) ? data.assets : [];

  if (!assets.length) {
    elements.releaseAssetsBody.innerHTML = `
      <tr>
        <td colspan="4" class="p-3 text-slate-500">${escapeHtml(t('rel.no_assets'))}</td>
      </tr>`;
    return;
  }

  const weight = (name) => {
    const lower = String(name).toLowerCase();
    if (lower.endsWith('setup.exe')) return 0;
    if (lower.endsWith('.msi')) return 1;
    if (lower.endsWith('.exe')) return 2;
    if (lower.endsWith('.appimage')) return 3;
    if (lower.endsWith('.deb')) return 4;
    if (lower.endsWith('.apk')) return 5;
    return 6;
  };

  const rows = assets
    .filter((asset) => /\.(exe|msi|apk|appimage|deb)$/i.test(asset.name || ''))
    .sort((a, b) => weight(a.name) - weight(b.name))
    .map((asset) => {
      const meta = releaseAssetMeta(asset.name);
      const size = formatBytes(asset.size);
      return `
        <tr class="hover:bg-surface-850/50">
          <td class="p-3 font-sans font-semibold text-slate-200 flex items-center gap-2">
            ${releaseIcon(meta.icon, meta.tone)}
            <span>${escapeHtml(meta.platform)}</span>
          </td>
          <td class="p-3 text-indigo-300 break-all">${escapeHtml(asset.name || '')}</td>
          <td class="p-3 text-slate-400">${escapeHtml(meta.arch)}</td>
          <td class="p-3 text-right whitespace-nowrap">
            <button data-open-url="${escapeHtml(asset.url || '')}" class="px-2.5 py-1 rounded bg-surface-800 hover:bg-surface-700 text-brand-300 border border-surface-700 transition-all">
              ${escapeHtml(t('rel.download_btn'))}${size ? ` · ${escapeHtml(size)}` : ''}
            </button>
          </td>
        </tr>`;
    });

  elements.releaseAssetsBody.innerHTML = rows.join('') || `
    <tr>
      <td colspan="4" class="p-3 text-slate-500">${escapeHtml(t('rel.no_assets'))}</td>
    </tr>`;
}

// ==================== COPIAS DE SEGURIDAD DEL CÓDIGO ====================
function refreshBackupExtensions() {
  if (!elements.selectBackupExt) return;
  const current = elements.selectBackupExt.value || state.selectedBackupExt;
  const options = [`<option value="">${escapeHtml(t('tools.select_ext'))}</option>`]
    .concat(state.extensions.map((ext) => (
      `<option value="${escapeHtml(ext.pkg_id)}">${escapeHtml(ext.name)} — ${escapeHtml(ext.pkg_id)}</option>`
    )))
    .join('');
  elements.selectBackupExt.innerHTML = options;
  if (current && state.extensions.some((ext) => ext.pkg_id === current)) {
    elements.selectBackupExt.value = current;
  }
}

async function loadBackups(pkgId) {
  if (!elements.backupsList) return;
  const pkg = pkgId || elements.selectBackupExt?.value || state.selectedBackupExt;
  state.selectedBackupExt = pkg || '';
  if (!pkg) {
    elements.backupsList.innerHTML = `<p class="text-xs text-slate-500">${escapeHtml(t('tools.select_ext'))}</p>`;
    return;
  }

  elements.backupsList.innerHTML = `<p class="text-xs text-slate-500">${escapeHtml(t('rel.loading'))}</p>`;
  try {
    const list = await invokeBackend('list_extension_backups', { pkgId: pkg }) || [];
    if (!list.length) {
      elements.backupsList.innerHTML = `<p class="text-xs text-slate-500">${escapeHtml(t('tools.backups_empty'))}</p>`;
      return;
    }
    elements.backupsList.innerHTML = list.map((backup) => `
      <div class="flex flex-wrap items-center justify-between gap-2 p-3 rounded-xl bg-surface-950 border border-surface-800">
        <div class="min-w-0">
          <p class="text-xs font-semibold text-slate-200">${escapeHtml(backup.created_at || backup.name)}</p>
          <p class="text-[10px] text-slate-500 font-mono">${escapeHtml(backup.name)} · ${escapeHtml(backup.reason || '')} · ${backup.files || 0} ${escapeHtml(t('tools.files'))}</p>
        </div>
        <button data-restore-backup="${escapeHtml(backup.name)}" class="px-3 py-1.5 rounded-lg text-[11px] font-medium bg-surface-800 hover:bg-surface-700 text-brand-300 border border-surface-700 transition-all">
          ${escapeHtml(t('tools.restore'))}
        </button>
      </div>
    `).join('');
  } catch (error) {
    elements.backupsList.innerHTML = `<p class="text-xs text-rose-400">${escapeHtml(String(error))}</p>`;
  }
}

async function openExtensionFolder(pkgId) {
  if (!pkgId) return;
  try {
    const info = await invokeBackend('open_extension_folder', { pkgId });
    appendTerminalLog('INFO', t('logs.folder', { path: info.path }));
    if (!info.opened) alert(t('alerts.path', { path: info.path }));
    return info;
  } catch (error) {
    alert(String(error));
    return null;
  }
}

// ==================== COMPILACIÓN LOCAL ====================
async function loadToolchain(force = false) {
  if (state.toolchain && !force) {
    renderToolchainStatus();
    return state.toolchain;
  }
  try {
    state.toolchain = await invokeBackend('check_build_toolchain');
  } catch (error) {
    state.toolchain = {
      can_build: false,
      missing: [String(error)],
      notes: [],
      java: false,
      kotlinc: false,
      gradle: false,
      android_sdk: false,
    };
  }
  renderToolchainStatus();
  return state.toolchain;
}

function renderToolchainStatus() {
  if (!elements.successToolchainStatus) return;
  const info = state.toolchain;
  if (!info) {
    elements.successToolchainStatus.textContent = t('succ.toolchain_checking');
    return;
  }

  const marks = [
    `Java ${info.java ? '✓' : '✗'}`,
    `Kotlin ${info.kotlinc ? '✓' : '—'}`,
    `Gradle ${info.gradle ? '✓' : '✗'}`,
    `Android SDK ${info.android_sdk ? '✓' : '✗'}`,
  ].join('  ·  ');

  if (info.can_build) {
    elements.successToolchainStatus.textContent = `${marks}\n${t('succ.toolchain_ready')}`;
    elements.successToolchainStatus.className = 'text-[10px] text-emerald-300 leading-relaxed whitespace-pre-line';
  } else {
    const missing = (info.missing || []).join(', ');
    elements.successToolchainStatus.textContent = `${marks}\n${t('succ.toolchain_missing')} ${missing}`;
    elements.successToolchainStatus.className = 'text-[10px] text-amber-300 leading-relaxed whitespace-pre-line';
  }

  if (elements.btnSuccessCompile) {
    // No se desactiva: si falta alguna herramienta, al pulsar se explica cuál
    // falta y se vuelve a comprobar (el usuario puede instalarla sin reiniciar).
    elements.btnSuccessCompile.disabled = false;
    elements.btnSuccessCompile.title = info.can_build
      ? t('succ.toolchain_ready')
      : `${t('succ.toolchain_missing')} ${(info.missing || []).join(', ')}`;
  }
}

async function compileSuccessExtension() {
  const ext = state.successExt;
  if (!ext || !elements.btnSuccessCompile) return;

  // Comprobación fresca: el usuario puede haber instalado el SDK tras abrir la app.
  const info = await loadToolchain(true);
  if (!info.can_build) {
    alert(t('alerts.compile_missing', { missing: (info.missing || []).join(', ') }));
    return;
  }

  const button = elements.btnSuccessCompile;
  const label = button.querySelector('span');
  const original = label ? label.textContent : '';
  button.disabled = true;
  button.classList.add('opacity-60', 'cursor-wait');
  if (label) label.textContent = t('succ.compiling');
  if (elements.successCompileOutput) {
    elements.successCompileOutput.classList.remove('hidden');
    elements.successCompileOutput.textContent = t('logs.compile_start', { pkg: ext.pkg_id });
  }
  appendTerminalLog('INFO', t('logs.compile_start', { pkg: ext.pkg_id }));

  try {
    const result = await invokeBackend('compile_extension', { pkgId: ext.pkg_id });
    if (elements.successCompileOutput) {
      elements.successCompileOutput.textContent = result.output || '';
      elements.successCompileOutput.classList.remove('hidden');
    }
    if (result.success) {
      appendTerminalLog('SUCCESS', t('logs.compile_ok', { path: result.apk_path || '' }));
      if (elements.successDownloadStatus) {
        elements.successDownloadStatus.classList.remove('hidden');
        elements.successDownloadStatus.textContent = `APK: ${result.apk_path}`;
      }
      await openExtensionFolder(ext.pkg_id);
    } else {
      appendTerminalLog('ERROR', t('logs.compile_fail', { pkg: ext.pkg_id }));
      alert(t('alerts.compile_failed'));
    }
  } catch (error) {
    if (elements.successCompileOutput) {
      elements.successCompileOutput.classList.remove('hidden');
      elements.successCompileOutput.textContent = String(error);
    }
    appendTerminalLog('ERROR', String(error));
    alert(String(error));
  } finally {
    button.disabled = false;
    button.classList.remove('opacity-60', 'cursor-wait');
    if (label) label.textContent = original || t('succ.compile');
  }
}

// Re-renderiza el contenido dinámico al cambiar el idioma.
window.addEventListener('aext:language', () => {
  if (!state.currentUser && elements.authBtnText) {
    elements.authBtnText.setAttribute('data-i18n', 'header.connect_github');
    elements.authBtnText.textContent = t('header.connect_github');
  }
  if (elements.selectExistingRepo && elements.selectExistingRepo.options.length) {
    syncRepoSelection();
  }
  if (state.extensions.length) renderExtensionsGrid(state.extensions);
  if (state.analyzedSites.length) {
    elements.analysisCards.innerHTML = '';
    state.analyzedSites.forEach(renderAnalysisCard);
  }
  renderReleaseTable(state.lastRelease);
  refreshBackupExtensions();
  if (state.selectedBackupExt) loadBackups(state.selectedBackupExt);
  renderToolchainStatus();
});


// ==================== NAVIGATION TABS ====================
elements.tabBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    const targetTab = btn.getAttribute('data-tab');
    elements.tabBtns.forEach(b => {
      b.classList.remove('active', 'border-brand-500', 'text-brand-400');
      b.classList.add('border-transparent', 'text-slate-400');
    });
    btn.classList.add('active', 'border-brand-500', 'text-brand-400');
    btn.classList.remove('border-transparent', 'text-slate-400');

    elements.tabPanes.forEach(pane => {
      if (pane.id === targetTab) {
        pane.classList.remove('hidden');
      } else {
        pane.classList.add('hidden');
      }
    });

    state.activeTab = targetTab;
    if (targetTab === 'tab-extensions') {
      loadExtensions();
    }
  });
});

// ==================== GITHUB AUTHENTICATION ====================
async function checkAuthStatus() {
  try {
    const user = await invokeBackend('get_gh_auth_status');
    updateAuthUI(user);
  } catch (err) {
    console.warn('Error verificando sesión GitHub:', err);
    updateAuthUI(null);
  }
}

function updateAuthUI(user) {
  state.currentUser = user;
  if (user) {
    // Al iniciar sesión el botón muestra el usuario, no el texto traducible.
    elements.authBtnText.removeAttribute('data-i18n');
    elements.authBtnText.textContent = `@${user.username}`;
    if (user.avatar_url) {
      elements.headerAvatar.src = user.avatar_url;
      elements.headerAvatar.classList.remove('hidden');
    }
    // Modal
    elements.authLoggedInView.classList.remove('hidden');
    elements.authLoggedOutView.classList.add('hidden');
    elements.modalUserAvatar.src = user.avatar_url || 'https://github.com/ghost.png';
    elements.modalUserName.textContent = user.display_name || user.username;
    elements.modalUserLogin.textContent = `@${user.username}`;
    elements.modalAuthTypeBadge.textContent = user.auth_type === 'cli' ? 'Sesión CLI' : 'Token PAT';
  } else {
    elements.authBtnText.setAttribute('data-i18n', 'header.connect_github');
    elements.authBtnText.textContent = t('header.connect_github');
    elements.headerAvatar.classList.add('hidden');
    elements.authLoggedInView.classList.add('hidden');
    elements.authLoggedOutView.classList.remove('hidden');
  }
}

elements.btnOpenAuthModal.addEventListener('click', () => {
  elements.modalGithubAuth.classList.remove('hidden');
  elements.modalGithubAuth.classList.add('flex');
});

elements.btnCloseAuthModal.addEventListener('click', () => {
  elements.modalGithubAuth.classList.add('hidden');
  elements.modalGithubAuth.classList.remove('flex');
});

if (elements.btnLoginCli) {
  elements.btnLoginCli.addEventListener('click', async () => {
    try {
      appendTerminalLog('INFO', 'Buscando credenciales en GitHub CLI local...');
      const info = await invokeBackend('login_with_cli');
      updateAuthUI(info);
      appendTerminalLog('SUCCESS', `Sesión iniciada con CLI: @${info.username}`);
      elements.modalGithubAuth.classList.add('hidden');
      elements.modalGithubAuth.classList.remove('flex');
    } catch (e) {
      appendTerminalLog('WARN', `CLI no detectado: ${e}. Ingresa un Personal Access Token.`);
      alert(t('alerts.cli_missing', { error: e }));
    }
  });
}

elements.btnSubmitPat.addEventListener('click', async () => {
  const token = elements.inputPatToken.value.trim();
  if (!token) {
    alert(t('alerts.token_required'));
    return;
  }
  try {
    appendTerminalLog('INFO', 'Verificando token con la API de GitHub...');
    const info = await invokeBackend('login_with_token', { token });
    updateAuthUI(info);
    appendTerminalLog('SUCCESS', `Sesión iniciada con éxito para @${info.username}`);
    elements.inputPatToken.value = '';
    elements.modalGithubAuth.classList.add('hidden');
    elements.modalGithubAuth.classList.remove('flex');
  } catch (e) {
    appendTerminalLog('ERROR', `Error de autenticación: ${e}`);
    alert(t('alerts.token_invalid', { error: e }));
  }
});

elements.btnSwitchAccount.addEventListener('click', () => {
  elements.authLoggedInView.classList.add('hidden');
  elements.authLoggedOutView.classList.remove('hidden');
});

elements.btnLogout.addEventListener('click', async () => {
  try {
    await invokeBackend('logout_gh');
    updateAuthUI(null);
    appendTerminalLog('INFO', 'Sesión de GitHub cerrada');
  } catch (e) {
    console.error(e);
  }
});

// ==================== URL ANALYZER ====================
function getEnteredUrls() {
  const text = elements.inputUrls.value;
  return text.split('\n')
    .map(u => u.trim())
    .filter(u => u.startsWith('http://') || u.startsWith('https://'));
}

elements.btnAnalyzeUrls.addEventListener('click', async () => {
  const urls = getEnteredUrls();
  if (urls.length === 0) {
    alert(t('alerts.urls_required'));
    return;
  }

  appendTerminalLog('INFO', `Iniciando análisis determinista para ${urls.length} sitio(s)...`);
  elements.analysisCards.innerHTML = '';
  elements.analysisPreviewContainer.classList.remove('hidden');
  state.analyzedSites = [];

  for (const url of urls) {
    try {
      appendTerminalLog('INFO', `Analizando DOM y CMS en: ${url}`);
      const result = await invokeBackend('analyze_url', { url });
      state.analyzedSites.push(result);
      renderAnalysisCard(result);
      appendTerminalLog('SUCCESS', `Sitio identificado: ${result.name} (CMS: ${result.cms}, Servidores: ${result.detected_hosts.join(', ') || 'N/A'})`);
      for (const warning of (result.warnings || [])) {
        appendTerminalLog('WARN', `${result.name}: ${warning}`);
      }
    } catch (e) {
      appendTerminalLog('ERROR', `Fallo al analizar ${url}: ${e}`);
    }
  }
});

function renderAnalysisCard(site) {
  const card = document.createElement('div');
  card.className = 'p-4 rounded-xl bg-surface-950 border border-surface-800 space-y-3';

  const hostsBadges = site.detected_hosts.length > 0
    ? site.detected_hosts.map(h => `<span class="px-1.5 py-0.5 rounded text-[10px] bg-indigo-500/10 text-indigo-300 border border-indigo-500/20">${escapeHtml(h)}</span>`).join(' ')
    : '<span class="text-slate-500 text-[10px]">No detectados</span>';

  const protectionsBadges = site.protections.length > 0
    ? site.protections.map(p => `<span class="px-1.5 py-0.5 rounded text-[10px] bg-amber-500/10 text-amber-300 border border-amber-500/20">${escapeHtml(p)}</span>`).join(' ')
    : '<span class="text-slate-500 text-[10px]">Ninguna</span>';

  const warnings = Array.isArray(site.warnings) ? site.warnings : [];
  const samples = Array.isArray(site.sample_items) ? site.sample_items : [];
  const searchValue = site.search_path
    ? (site.search_param ? `${site.search_path} · ${site.search_param}` : site.search_path)
    : null;
  const routes = [
    { label: t('analysis.listing'), value: site.popular_path },
    { label: t('analysis.latest'), value: site.latest_path },
    { label: t('analysis.search'), value: searchValue },
  ];
  const routesHtml = routes.map(route => `
      <div>
        <span class="text-slate-500 block text-[10px]">${escapeHtml(route.label)}</span>
        <span class="font-mono text-[10px] ${route.value ? 'text-emerald-300' : 'text-slate-500'}">${route.value ? escapeHtml(route.value) : escapeHtml(t('analysis.not_detected'))}</span>
      </div>`).join('');

  const confidenceLabel = site.catalog_detected
    ? t('analysis.confidence_ok')
    : t('analysis.confidence_low');
  const confidenceClass = site.catalog_detected
    ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/20'
    : 'bg-amber-500/10 text-amber-300 border-amber-500/20';

  const samplesHtml = samples.length > 0
    ? `<div class="space-y-1">
        <span class="text-slate-500 block text-[10px]">${escapeHtml(t('analysis.samples'))}</span>
        <div class="flex flex-wrap gap-1">${samples.map(s => `<span class="px-1.5 py-0.5 rounded text-[10px] bg-surface-800 text-slate-300 border border-surface-700">${escapeHtml(s)}</span>`).join('')}</div>
      </div>`
    : '';

  const warningsHtml = warnings.length > 0
    ? `<div class="space-y-1 pt-2 border-t border-amber-500/20">
        <span class="text-amber-300 block text-[10px] font-semibold">${escapeHtml(t('analysis.warnings'))}</span>
        <ul class="list-disc list-inside space-y-0.5 text-[10px] text-amber-200/80">
          ${warnings.map(w => `<li>${escapeHtml(w)}</li>`).join('')}
        </ul>
      </div>`
    : '';

  const siteIcon = site.favicon_url && site.favicon_url.trim()
    ? site.favicon_url
    : fallbackIcon(site.name);
  const siteIconFallback = fallbackIcon(site.name);

  card.innerHTML = `
    <div class="flex items-start justify-between gap-3">
      <div class="flex items-center gap-3">
        <img class="w-8 h-8 rounded-lg bg-surface-800 object-cover" src="${escapeHtml(siteIcon)}" onerror="this.onerror=null;this.src='${siteIconFallback}'">
        <div>
          <h4 class="text-xs font-bold text-slate-100">${escapeHtml(site.name)}</h4>
          <span class="text-[10px] font-mono text-slate-400" title="${escapeHtml(site.entry_url || site.url)}">${escapeHtml(site.url)}</span>
        </div>
      </div>
      <div class="flex flex-col items-end gap-1">
        <span class="px-2 py-0.5 rounded text-[10px] font-semibold bg-brand-500/10 text-brand-300 border border-brand-500/20 uppercase">${escapeHtml(site.ext_type)}</span>
        <span class="px-2 py-0.5 rounded text-[10px] font-semibold border ${confidenceClass}">${escapeHtml(confidenceLabel)}</span>
      </div>
    </div>

    <div class="grid grid-cols-2 gap-2 text-[11px] pt-1">
      <div>
        <span class="text-slate-500 block text-[10px]">Motor / CMS</span>
        <span class="font-medium text-slate-300 capitalize">${escapeHtml(site.cms)}</span>
      </div>
      <div>
        <span class="text-slate-500 block text-[10px]">Doblaje (SUB/DUB)</span>
        <span class="font-medium ${site.has_sub_dub ? 'text-emerald-400' : 'text-slate-400'}">${site.has_sub_dub ? 'Detectado en Embeds' : 'No explícito'}</span>
      </div>
    </div>

    <div class="space-y-1 pt-2 border-t border-surface-800">
      <span class="text-slate-500 block text-[10px]">${escapeHtml(t('analysis.detected_paths'))}</span>
      <div class="grid grid-cols-3 gap-2">${routesHtml}</div>
      <div class="pt-1">
        <span class="text-slate-500 block text-[10px]">${escapeHtml(t('analysis.episodes'))}</span>
        <span class="font-mono text-[10px] ${site.episode_selector ? 'text-emerald-300' : 'text-amber-300'}">${escapeHtml(site.episode_selector && site.episode_selector.trim() ? site.episode_selector : t('analysis.episodes_generic'))}</span>
      </div>
    </div>

    ${samplesHtml}

    <div class="space-y-1 pt-1 border-t border-surface-800">
      <span class="text-slate-500 block text-[10px]">Servidores detectados:</span>
      <div class="flex flex-wrap gap-1">${hostsBadges}</div>
    </div>

    <div class="space-y-1">
      <span class="text-slate-500 block text-[10px]">Protecciones WAF / Cloudflare:</span>
      <div class="flex flex-wrap gap-1">${protectionsBadges}</div>
    </div>

    ${warningsHtml}
  `;

  elements.analysisCards.appendChild(card);
}

// ==================== GENERATION FLOW & DECISION MODAL ====================

function guessNameFromInput() {
  if (state.analyzedSites.length > 0 && state.analyzedSites[0].name) {
    return state.analyzedSites[0].name;
  }
  const urls = getEnteredUrls();
  if (urls.length === 0) return '';
  try {
    const host = new URL(urls[0]).hostname.replace(/^www\./, '');
    const label = host.split('.')[0] || host;
    return label.charAt(0).toUpperCase() + label.slice(1);
  } catch (_) {
    return '';
  }
}

function openGenerationModal() {
  // Prellenar el nombre con el sitio analizado (el usuario puede cambiarlo).
  if (elements.inputConfirmExtName && !elements.inputConfirmExtName.value.trim()) {
    elements.inputConfirmExtName.value = guessNameFromInput();
  }
  if (elements.selectConfirmExtLang && elements.selectExtLang) {
    elements.selectConfirmExtLang.value = elements.selectExtLang.value || 'es';
  }
  if (elements.selectConfirmExtType) {
    const analyzedType = state.analyzedSites[0]?.ext_type;
    if (analyzedType === 'anime' || analyzedType === 'manga') {
      elements.selectConfirmExtType.value = analyzedType;
    }
  }
  elements.modalGenMode.classList.remove('hidden');
  elements.modalGenMode.classList.add('flex');

  const selectedMode = document.querySelector('input[name="gen-mode"]:checked')?.value;
  if (selectedMode === 'github') {
    elements.githubRepoExtraFields.classList.remove('hidden');
    // No se rellena el campo "otro repo" con el repositorio guardado: si está
    // en la lista, el selector lo muestra; si no, el usuario escribe uno.
    loadGithubRepos().then(syncRepoSelection);
  }
}

elements.btnStartGenFlow.addEventListener('click', () => {
  const urls = getEnteredUrls();
  if (urls.length === 0) {
    alert(t('alerts.urls_required'));
    return;
  }

  openGenerationModal();
});

elements.btnCancelGenMode.addEventListener('click', () => {
  elements.modalGenMode.classList.add('hidden');
  elements.modalGenMode.classList.remove('flex');
});

async function loadGithubRepos() {
  if (!elements.selectExistingRepo) return;
  if (!state.currentUser) {
    elements.selectExistingRepo.innerHTML = `<option value="">${escapeHtml(t('modal.repo_none'))}</option>`;
    syncRepoSelection();
    return;
  }
  elements.selectExistingRepo.innerHTML = `<option value="">${escapeHtml(t('modal.repo_loading'))}</option>`;
  try {
    const repos = await invokeBackend('list_github_repos') || [];
    state.githubRepos = repos;
    if (repos.length === 0) {
      elements.selectExistingRepo.innerHTML = `<option value="">${escapeHtml(t('modal.repo_none'))}</option>`;
      syncRepoSelection();
      return;
    }
    const saved = (localStorage.getItem('aext_default_repo') || '').toLowerCase();
    elements.selectExistingRepo.innerHTML = `<option value="">${escapeHtml(t('modal.repo_none'))}</option>` +
      repos.map(repo => {
        const value = escapeHtml(repo.full_name);
        const selected = repo.full_name.toLowerCase() === saved ? ' selected' : '';
        const visibility = repo.private ? ' (privado)' : '';
        return `<option value="${value}"${selected}>${escapeHtml(repo.full_name)}${visibility}</option>`;
      }).join('');
    syncRepoSelection();
  } catch (e) {
    console.warn('No se pudieron listar los repositorios:', e);
    elements.selectExistingRepo.innerHTML = `<option value="">${escapeHtml(t('modal.repo_none'))}</option>`;
    syncRepoSelection();
  }
}

/// Si hay un repositorio seleccionado, el campo "otro repo" queda desactivado
/// para que no haya dos destinos contradictorios. Con «Ninguno» se habilita.
function syncRepoSelection() {
  if (!elements.selectExistingRepo || !elements.inputCustomRepoName) return;
  const selected = elements.selectExistingRepo.value.trim();
  const input = elements.inputCustomRepoName;
  if (selected) {
    input.value = '';
    input.disabled = true;
    input.placeholder = t('modal.repo_using');
    input.classList.add('opacity-50', 'cursor-not-allowed');
  } else {
    input.disabled = false;
    input.placeholder = t('modal.repo_custom_ph');
    input.classList.remove('opacity-50', 'cursor-not-allowed');
  }
}

if (elements.selectExistingRepo) {
  elements.selectExistingRepo.addEventListener('change', syncRepoSelection);
}

if (elements.btnRefreshRepos) {
  elements.btnRefreshRepos.addEventListener('click', loadGithubRepos);
}

// Listener para mostrar/ocultar campos del repo
document.querySelectorAll('input[name="gen-mode"]').forEach(radio => {
  radio.addEventListener('change', (e) => {
    if (e.target.value === 'github') {
      elements.githubRepoExtraFields.classList.remove('hidden');
      loadGithubRepos().then(syncRepoSelection);
    } else {
      elements.githubRepoExtraFields.classList.add('hidden');
    }
  });
});

elements.btnConfirmGenMode.addEventListener('click', async () => {
  const urls = getEnteredUrls();
  const selectedMode = document.querySelector('input[name="gen-mode"]:checked').value;
  const selectedRepo = elements.selectExistingRepo?.value?.trim() || '';
  const typedRepo = elements.inputCustomRepoName?.value?.trim() || '';
  // «Ninguno» + campo vacío = crear repositorio nuevo. No se usa el repo
  // guardado en ajustes en silencio: solo se prellena y el usuario decide.
  const customRepoName = selectedRepo || typedRepo || null;
  const isPrivate = elements.checkPrivateRepo?.checked || false;

  const customName = elements.inputConfirmExtName?.value?.trim() || null;
  const extType = elements.selectConfirmExtType?.value || null;
  const selectedLang = elements.selectConfirmExtLang?.value
    || document.getElementById('select-extension-lang')?.value
    || 'es';

  // Si el análisis no pudo confirmar el catálogo (SPA, sin tarjetas, sin
  // buscador), pedir confirmación explícita: no se van a inventar rutas.
  const lowConfidence = state.analyzedSites.filter(
    site => site && (!site.catalog_detected || !site.popular_path || !site.search_path)
  );
  if (lowConfidence.length > 0) {
    const warningText = lowConfidence
      .map(site => {
        const detail = Array.isArray(site.warnings) && site.warnings.length
          ? site.warnings.join(' ')
          : t('analysis.confidence_low');
        return `• ${site.name}: ${detail}`;
      })
      .join('\n');
    if (!confirm(t('alerts.low_confidence', { warnings: warningText }))) {
      return;
    }
  }

  elements.modalGenMode.classList.add('hidden');
  elements.modalGenMode.classList.remove('flex');

  if (selectedMode === 'github' && !state.currentUser) {
    alert(t('alerts.github_required'));
    elements.modalGithubAuth.classList.remove('hidden');
    elements.modalGithubAuth.classList.add('flex');
    return;
  }

  appendTerminalLog('INFO', t('logs.gen_start', { mode: selectedMode.toUpperCase() }));
  appendTerminalLog('INFO', t('logs.gen_meta', {
    name: customName || guessNameFromInput() || '—',
    type: extType || 'auto',
    lang: selectedLang,
  }));

  try {
    const request = {
      urls: urls,
      mode: selectedMode,
      repo_name: customRepoName,
      private_repo: isPrivate,
      custom_name: customName,
      lang: selectedLang,
      nsfw: false,
      ext_type: extType
    };

    const createdExtensions = await invokeBackend('generate_extensions', { request });
    appendTerminalLog('SUCCESS', t('logs.gen_success', { count: createdExtensions.length }));

    for (const ext of createdExtensions) {
      if (ext.github_repo_url) {
        appendTerminalLog('SUCCESS', t('logs.repo', { url: ext.github_repo_url }));
      }
      if (ext.nekoyomi_repo_url) {
        appendTerminalLog('SUCCESS', t('logs.nekoyomi', { url: ext.nekoyomi_repo_url }));
      }
    }

    if (createdExtensions.length === 1) {
      showGenerationSuccess(createdExtensions[0]);
    } else {
      // Cambiar automáticamente a la pestaña de extensiones
      const tabExt = document.querySelector('[data-tab="tab-extensions"]');
      if (tabExt) tabExt.click();
    }

  } catch (err) {
    appendTerminalLog('ERROR', t('logs.gen_error', { error: err }));
    alert(`${t('logs.gen_error', { error: err })}`);
  }
});

// ==================== SUCCESS MODAL ====================

function showGenerationSuccess(ext) {
  if (!elements.modalGenSuccess) return;
  state.successExt = ext;

  elements.successExtIcon.src = iconFor(ext);
  elements.successExtIcon.onerror = () => {
    elements.successExtIcon.onerror = null;
    elements.successExtIcon.src = fallbackIcon(ext.name);
  };
  if (elements.successExtName) elements.successExtName.textContent = ext.name || 'Extensión';
  if (elements.successExtPkg) elements.successExtPkg.textContent = ext.pkg_id || '';
  if (elements.successExtType) elements.successExtType.textContent = (ext.ext_type || 'anime').toUpperCase();
  if (elements.successExtVersion) elements.successExtVersion.textContent = `v${ext.version || '1'}`;

  if (elements.successDownloadStatus) {
    elements.successDownloadStatus.classList.add('hidden');
    elements.successDownloadStatus.textContent = '';
  }
  if (elements.successCompileOutput) {
    elements.successCompileOutput.classList.add('hidden');
    elements.successCompileOutput.textContent = '';
  }
  if (elements.successBuildSection) {
    elements.successBuildSection.classList.remove('hidden');
  }
  if (elements.btnSuccessCompile) {
    elements.btnSuccessCompile.disabled = true;
    const label = elements.btnSuccessCompile.querySelector('span');
    if (label) label.textContent = t('succ.compile');
  }

  if (elements.btnSuccessInstallApk) {
    const button = elements.btnSuccessInstallApk;
    button.onclick = null;
    button.removeAttribute('target');
    button.removeAttribute('download');
    const label = elements.btnSuccessInstallLabel;

    if (ext.apk_download_url) {
      const isRealApk = /\.apk(\?|$)/i.test(ext.apk_download_url);
      if (isRealApk) {
        // APK real: se abre en el navegador para que el sistema lo descargue.
        button.href = ext.apk_download_url;
        button.classList.remove('hidden');
        if (label) label.textContent = t('succ.apk');
      } else {
        // ZIP local: descarga por Blob para no navegar fuera de la app.
        button.href = '#';
        button.onclick = async (event) => {
          event.preventDefault();
          const original = label ? label.textContent : t('succ.zip');
          button.classList.add('opacity-60', 'cursor-wait');
          if (label) label.textContent = t('ext.downloading');
          try {
            const saved = await saveLocalZip(ext.apk_download_url, `${ext.pkg_id}.zip`);
            appendTerminalLog('SUCCESS', t('logs.zip_saved', { file: `${ext.pkg_id}.zip` }));
            if (saved && saved.path) {
              appendTerminalLog('INFO', t('logs.zip_path', { path: saved.path }));
            }
            if (elements.successDownloadStatus) {
              elements.successDownloadStatus.classList.remove('hidden');
              elements.successDownloadStatus.textContent = saved && saved.path
                ? t('succ.downloaded_to', { path: saved.path })
                : t('succ.downloaded');
            }
            if (label) label.textContent = `✓ ${t('succ.zip')}`;
          } catch (error) {
            appendTerminalLog('ERROR', t('logs.zip_error', {
              file: `${ext.pkg_id}.zip`,
              error: error.message || error,
            }));
            alert(t('alerts.zip_failed', { error: error.message || error }));
          } finally {
            button.classList.remove('opacity-60', 'cursor-wait');
          }
        };
        button.classList.remove('hidden');
        if (label) label.textContent = t('succ.zip');
      }
    } else if (ext.github_repo_url) {
      button.href = `${ext.github_repo_url.replace(/\/$/, '')}/actions`;
      button.setAttribute('target', '_blank');
      button.classList.remove('hidden');
      if (label) label.textContent = t('succ.actions');
    } else {
      button.classList.add('hidden');
    }
  }

  if (elements.btnSuccessDeeplinkRepo) {
    if (ext.nekoyomi_repo_url) {
      elements.btnSuccessDeeplinkRepo.href = `nekoyomi://add-repo?url=${encodeURIComponent(ext.nekoyomi_repo_url)}`;
      elements.btnSuccessDeeplinkRepo.classList.remove('hidden');
    } else {
      elements.btnSuccessDeeplinkRepo.classList.add('hidden');
    }
  }

  if (elements.btnSuccessCopyRepo) {
    elements.btnSuccessCopyRepo.dataset.repoUrl = ext.nekoyomi_repo_url || ext.github_repo_url || '';
  }

  elements.modalGenSuccess.classList.remove('hidden');
  elements.modalGenSuccess.classList.add('flex');
  loadToolchain();
}

function closeGenerationSuccess() {
  elements.modalGenSuccess.classList.add('hidden');
  elements.modalGenSuccess.classList.remove('flex');
  state.successExt = null;
  const tabExt = document.querySelector('[data-tab="tab-extensions"]');
  if (tabExt) tabExt.click();
}

if (elements.btnCloseSuccessModal) {
  elements.btnCloseSuccessModal.addEventListener('click', closeGenerationSuccess);
}

if (elements.btnSuccessOpenFolder) {
  elements.btnSuccessOpenFolder.addEventListener('click', () => {
    if (state.successExt) openExtensionFolder(state.successExt.pkg_id);
  });
}

if (elements.btnSuccessCompile) {
  elements.btnSuccessCompile.addEventListener('click', compileSuccessExtension);
}

if (elements.btnSuccessCopyRepo) {
  elements.btnSuccessCopyRepo.addEventListener('click', async () => {
    const url = elements.btnSuccessCopyRepo.dataset.repoUrl;
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
      alert(t('alerts.copy_repo'));
    } catch (_) {
      alert(t('alerts.copy_repo_fallback', { url }));
    }
  });
}

// ==================== EXTENSIONS CATALOG ====================
async function loadExtensions() {
  try {
    const list = await invokeBackend('get_created_extensions') || [];
    state.extensions = list;
    elements.badgeExtCount.textContent = list.length;
    renderExtensionsGrid(list);
  } catch (e) {
    console.error('Error cargando extensiones:', e);
  }
}

function renderExtensionsGrid(list) {
  if (list.length === 0) {
    elements.extensionsCatalogGrid.innerHTML = `
      <div class="col-span-full py-12 text-center text-slate-500 text-xs">
        ${escapeHtml(t('ext.empty'))}
      </div>
    `;
    return;
  }

  elements.extensionsCatalogGrid.innerHTML = '';
  for (const ext of list) {
    const card = document.createElement('div');
    card.className = 'p-4 rounded-2xl bg-surface-900 border border-surface-800 space-y-4 hover:border-slate-700 transition-all shadow-lg';

    const icon = iconFor(ext);
    const iconFallback = fallbackIcon(ext.name);
    const downloadUrl = ext.apk_download_url || '';
    const isRealApk = /\.apk(\?|$)/i.test(downloadUrl);
    const hasGithub = Boolean(ext.github_repo_url);

    let primaryAction;
    if (downloadUrl && isRealApk) {
      primaryAction = `
        <a href="${escapeHtml(downloadUrl)}" class="flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-brand-600 hover:bg-brand-500 text-white flex items-center justify-center gap-1.5 transition-all text-center">
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" /></svg>
          <span>${escapeHtml(t('succ.apk'))}</span>
        </a>`;
    } else if (downloadUrl) {
      primaryAction = `
        <button data-download-url="${escapeHtml(downloadUrl)}" data-download-name="${escapeHtml(ext.pkg_id)}.zip" class="btn-download-zip flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-brand-600 hover:bg-brand-500 text-white flex items-center justify-center gap-1.5 transition-all text-center">
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" /></svg>
          <span>${escapeHtml(t('ext.download_zip'))}</span>
        </button>`;
    } else if (hasGithub) {
      primaryAction = `
        <a href="${escapeHtml(ext.github_repo_url.replace(/\/$/, ''))}/actions" target="_blank" rel="noreferrer" class="flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-brand-600 hover:bg-brand-500 text-white flex items-center justify-center gap-1.5 transition-all text-center" title="La compilación del APK corre en GitHub Actions">
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" /></svg>
          <span>${escapeHtml(t('ext.actions'))}</span>
        </a>`;
    } else {
      primaryAction = `
        <span class="flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-surface-800 text-slate-500 flex items-center justify-center gap-1.5 text-center">
          ${escapeHtml(t('ext.no_download'))}
        </span>`;
    }

    card.innerHTML = `
      <div class="flex items-start justify-between">
        <div class="flex items-center gap-3">
          <img class="w-11 h-11 rounded-xl bg-surface-950 object-cover ring-1 ring-white/10" src="${escapeHtml(icon)}" onerror="this.onerror=null;this.src='${iconFallback}'" alt="${escapeHtml(ext.name)}">
          <div>
            <h3 class="text-sm font-bold text-slate-100 flex items-center gap-1.5">
              <span>${escapeHtml(ext.name)}</span>
              <span class="text-[10px] font-mono px-1.5 py-0.2 rounded bg-surface-800 text-slate-300 font-normal">v${escapeHtml(ext.version)}</span>
            </h3>
            <p class="text-[11px] font-mono text-slate-400">${escapeHtml(ext.pkg_id)}</p>
          </div>
        </div>
        <span class="px-2 py-0.5 rounded text-[10px] font-semibold bg-brand-500/10 text-brand-300 border border-brand-500/20 uppercase">${escapeHtml(ext.ext_type)}</span>
      </div>

      <div class="grid grid-cols-2 gap-2 text-[11px] pt-1 border-t border-surface-800/60">
        <div>
          <span class="text-slate-500 block text-[10px]">${escapeHtml(t('ext.cms'))}</span>
          <span class="text-slate-300 font-medium capitalize">${escapeHtml(ext.cms)}</span>
        </div>
        <div>
          <span class="text-slate-500 block text-[10px]">${escapeHtml(t('ext.audio'))}</span>
          <span class="text-slate-300 font-medium">${ext.has_sub_dub ? escapeHtml(t('ext.audio_config')) : escapeHtml(t('ext.audio_standard'))}</span>
        </div>
      </div>

      <!-- Action Buttons -->
      <div class="pt-2 border-t border-surface-800 flex flex-wrap items-center gap-2">
        ${primaryAction}

        <button data-update-pkg="${escapeHtml(ext.pkg_id)}" class="btn-update-ext py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700 flex items-center gap-1.5 transition-all" title="${escapeHtml(t('ext.update'))}">
          <svg class="w-3.5 h-3.5 text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" /></svg>
          <span>${escapeHtml(t('ext.update'))}</span>
        </button>

        <button data-open-folder-pkg="${escapeHtml(ext.pkg_id)}" class="btn-open-folder py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700 flex items-center gap-1.5 transition-all" title="${escapeHtml(t('ext.open_folder'))}">
          <svg class="w-3.5 h-3.5 text-brand-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/></svg>
          <span>${escapeHtml(t('ext.open_folder'))}</span>
        </button>

        ${ext.nekoyomi_repo_url ? `
          <button data-repo-url="${escapeHtml(ext.nekoyomi_repo_url)}" class="btn-copy-repo py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700 flex items-center gap-1.5 transition-all" title="Copiar enlace de repositorio para Nekoyomi">
            <svg class="w-3.5 h-3.5 text-brand-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" /></svg>
            <span>${escapeHtml(t('ext.copy_repo'))}</span>
          </button>
        ` : ''}

        ${ext.github_repo_url ? `
          <a href="${escapeHtml(ext.github_repo_url)}" target="_blank" class="py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-300 border border-surface-700 flex items-center gap-1 transition-all" title="${escapeHtml(t('ext.github'))}">
            <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24"><path fill-rule="evenodd" clip-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" /></svg>
          </a>
        ` : ''}

        <!-- Delete button -->
        <button data-delete-pkg="${escapeHtml(ext.pkg_id)}" class="btn-delete-ext p-2 rounded-xl text-slate-500 hover:text-red-400 hover:bg-red-500/10 transition-all" title="${escapeHtml(t('ext.delete'))}">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
        </button>
      </div>
    `;

    elements.extensionsCatalogGrid.appendChild(card);
  }

  // Hook copy buttons
  document.querySelectorAll('.btn-copy-repo').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const url = e.currentTarget.getAttribute('data-repo-url');
      if (!url) return;
      try {
        await navigator.clipboard.writeText(url);
        alert(t('alerts.copy_repo'));
      } catch (_) {
        alert(t('alerts.copy_repo_fallback', { url }));
      }
    });
  });

  // Hook open-folder buttons
  document.querySelectorAll('.btn-open-folder').forEach(btn => {
    btn.addEventListener('click', (e) => {
      const pkgId = e.currentTarget.getAttribute('data-open-folder-pkg');
      if (pkgId) openExtensionFolder(pkgId);
    });
  });

  // Hook local ZIP download buttons (Blob: nunca navega fuera de la app)
  document.querySelectorAll('.btn-download-zip').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const button = e.currentTarget;
      const url = button.getAttribute('data-download-url');
      const filename = button.getAttribute('data-download-name') || 'extension.zip';
      if (!url) return;
      const label = button.querySelector('span');
      const original = label ? label.textContent : '';
      button.disabled = true;
      button.classList.add('opacity-60', 'cursor-wait');
      if (label) label.textContent = t('ext.downloading');
      try {
        const saved = await saveLocalZip(url, filename);
        appendTerminalLog('SUCCESS', t('logs.zip_saved', { file: filename }));
        if (saved && saved.path) {
          appendTerminalLog('INFO', t('logs.zip_path', { path: saved.path }));
        }
        if (label) label.textContent = `✓ ${t('ext.download_zip')}`;
      } catch (error) {
        appendTerminalLog('ERROR', t('logs.zip_error', { file: filename, error: error.message || error }));
        alert(t('alerts.zip_failed', { error: error.message || error }));
        if (label) label.textContent = original || t('ext.download_zip');
      } finally {
        button.disabled = false;
        button.classList.remove('opacity-60', 'cursor-wait');
      }
    });
  });

  // Hook update buttons
  document.querySelectorAll('.btn-update-ext').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const button = e.currentTarget;
      const pkgId = button.getAttribute('data-update-pkg');
      if (!pkgId) return;
      const label = button.querySelector('span');
      const original = label ? label.textContent : '';
      button.disabled = true;
      button.classList.add('opacity-60', 'cursor-wait');
      if (label) label.textContent = t('ext.updating');
      appendTerminalLog('INFO', `↻ ${pkgId}…`);
      try {
        const updated = await invokeBackend('update_extension', { pkgId });
        appendTerminalLog('SUCCESS', t('logs.updated', {
          name: updated ? updated.name : pkgId,
          version: updated ? updated.version : '?',
        }));
        await loadExtensions();
        refreshBackupExtensions();
      } catch (err) {
        appendTerminalLog('ERROR', t('logs.update_error', { pkg: pkgId, error: err }));
        alert(t('logs.update_error', { pkg: pkgId, error: err }));
        button.disabled = false;
        button.classList.remove('opacity-60', 'cursor-wait');
        if (label) label.textContent = original || t('ext.update');
      }
    });
  });

  // Hook delete buttons
  document.querySelectorAll('.btn-delete-ext').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const pkgId = e.currentTarget.getAttribute('data-delete-pkg');
      if (confirm(t('alerts.confirm_delete', { pkg: pkgId }))) {
        await invokeBackend('delete_extension', { pkgId });
        loadExtensions();
        refreshBackupExtensions();
      }
    });
  });
}

elements.btnRefreshExtensions.addEventListener('click', loadExtensions);

// ==================== STREAM EXTRACTOR TESTER & DIAGNOSTICS ====================
elements.btnTestStream.addEventListener('click', async () => {
  const url = elements.testStreamInput.value.trim();
  if (!url) {
    alert(t('alerts.no_stream'));
    return;
  }

  elements.testStreamResult.classList.remove('hidden');
  elements.testStreamResult.innerHTML = `<span class="text-slate-400">Ejecutando prueba de conexión y bypass de cabeceras...</span>`;

  try {
    const res = await invokeBackend('test_stream_extractor', { url });
    if (res.success) {
      elements.testStreamResult.innerHTML = `
        <div class="text-emerald-400 font-bold">[OK] ÉXITO: El servidor autorizó la reproducción (${res.host})</div>
        <div class="text-slate-300">Calidad: ${res.quality || 'Automática'}</div>
        <div class="text-slate-500 text-[10px] mt-1">Cabeceras enviadas: Referer, Origin, Sec-Fetch-Mode: cors</div>
      `;
    } else {
      elements.testStreamResult.innerHTML = `
        <div class="text-rose-400 font-bold">[FALLO]: ${res.error || 'Acceso denegado'} (${res.host})</div>
        <div class="text-slate-400 text-[10px]">Causa probable: Cloudflare WAF requiere cookies o token de sesión temporal.</div>
      `;
    }
  } catch (e) {
    elements.testStreamResult.innerHTML = `<div class="text-rose-400">Error: ${e}</div>`;
  }
});

// ==================== HERRAMIENTAS, MCP Y COPIAS DE SEGURIDAD ====================
if (elements.btnCopyMcpConfig) {
  elements.btnCopyMcpConfig.addEventListener('click', () => {
    const config = {
      mcpServers: {
        aext: {
          url: "http://127.0.0.1:45731/mcp"
        }
      }
    };
    navigator.clipboard.writeText(JSON.stringify(config, null, 2));
    alert(t('alerts.mcp_copied'));
  });
}

if (elements.btnCopyLogsAi) {
  elements.btnCopyLogsAi.addEventListener('click', async () => {
    try {
      const logs = await invokeBackend('get_system_logs') || [];
      const text = logs.map(l => `[${l.timestamp}] [${l.level}] ${l.message}`).join('\n');
      navigator.clipboard.writeText(`LOGS AEXT:\n${text}`);
      alert(t('alerts.logs_copied'));
    } catch (e) {
      alert(t('alerts.logs_copy_error'));
    }
  });
}

if (elements.btnGenAiDiag) {
  elements.btnGenAiDiag.addEventListener('click', async () => {
    try {
      const res = await fetch('http://127.0.0.1:45731/api/ai/diagnostics');
      const diag = await res.json();
      navigator.clipboard.writeText(JSON.stringify(diag, null, 2));
      alert(t('alerts.diag_copied'));
    } catch (e) {
      alert(t('alerts.diag_error'));
    }
  });
}

if (elements.btnApplyAiPatch) {
  elements.btnApplyAiPatch.addEventListener('click', async () => {
    const val = elements.inputAiPatch.value.trim();
    if (!val) {
      alert('Ingresa el JSON del parche');
      return;
    }
    try {
      const patch = JSON.parse(val);
      const res = await fetch('http://127.0.0.1:45731/api/ai/patch', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(patch)
      });
      const data = await res.json();
      if (res.ok) {
        appendTerminalLog('SUCCESS', t('logs.patch_ok', { pkg: patch.pkg_id, version: data.version }));
        alert(t('alerts.patch_ok', {
          version: data.version,
          backup: data.backup || '—',
        }));
        const pkgId = patch.pkg_id;
        await loadExtensions();
        refreshBackupExtensions();
        if (pkgId) loadBackups(pkgId);
      } else {
        alert(t('alerts.patch_error', { error: data.error }));
      }
    } catch (e) {
      alert(t('alerts.invalid_json', { error: e.message }));
    }
  });
}

if (elements.selectBackupExt) {
  elements.selectBackupExt.addEventListener('change', () => {
    state.selectedBackupExt = elements.selectBackupExt.value;
    loadBackups(state.selectedBackupExt);
  });
}

if (elements.btnRefreshBackups) {
  elements.btnRefreshBackups.addEventListener('click', async () => {
    refreshBackupExtensions();
    await loadBackups(elements.selectBackupExt?.value || '');
  });
}

if (elements.btnOpenSelectedFolder) {
  elements.btnOpenSelectedFolder.addEventListener('click', () => {
    const pkgId = elements.selectBackupExt?.value || state.selectedBackupExt;
    if (!pkgId) {
      alert(t('alerts.missing_ext'));
      return;
    }
    openExtensionFolder(pkgId);
  });
}

if (elements.backupsList) {
  elements.backupsList.addEventListener('click', async (event) => {
    const button = event.target.closest('[data-restore-backup]');
    if (!button) return;
    const backup = button.getAttribute('data-restore-backup');
    const pkgId = elements.selectBackupExt?.value || state.selectedBackupExt;
    if (!backup || !pkgId) return;

    if (!confirm(`Restaurar ${backup} en ${pkgId}? Los archivos actuales se sobrescribirán.`)) {
      return;
    }

    button.disabled = true;
    try {
      const result = await invokeBackend('restore_extension_backup', { pkgId, backup });
      appendTerminalLog('SUCCESS', t('logs.restore_ok', {
        backup,
        count: result.restored_files,
      }));
      alert(t('alerts.restore_ok', { count: result.restored_files }));
      await loadExtensions();
      await loadBackups(pkgId);
    } catch (error) {
      appendTerminalLog('ERROR', String(error));
      alert(t('alerts.restore_error', { error }));
      button.disabled = false;
    }
  });
}

// ==================== IDIOMA DE LA INTERFAZ ====================
const initialUiLang = initLanguage();
if (elements.settingsAppUiLang) {
  elements.settingsAppUiLang.value = initialUiLang;
  elements.settingsAppUiLang.addEventListener('change', () => {
    setLanguage(elements.settingsAppUiLang.value);
  });
}

// ==================== SETTINGS SYNC ====================
// Sync language selector between Generator tab and Settings tab
if (elements.settingsLang && elements.selectExtLang) {
  // Load saved language from localStorage
  const savedLang = localStorage.getItem('aext_lang') || 'es';
  elements.settingsLang.value = savedLang;
  elements.selectExtLang.value = savedLang;

  elements.settingsLang.addEventListener('change', () => {
    elements.selectExtLang.value = elements.settingsLang.value;
  });
  elements.selectExtLang.addEventListener('change', () => {
    elements.settingsLang.value = elements.selectExtLang.value;
  });
}

// Load saved default repo
if (elements.settingsDefaultRepo) {
  const savedRepo = localStorage.getItem('aext_default_repo') || '';
  elements.settingsDefaultRepo.value = savedRepo;
}

// Save settings button
if (elements.btnSaveSettings) {
  elements.btnSaveSettings.addEventListener('click', () => {
    const lang = elements.settingsLang?.value || 'es';
    const repo = elements.settingsDefaultRepo?.value?.trim() || '';
    const uiLang = elements.settingsAppUiLang?.value || getLanguage();

    localStorage.setItem('aext_lang', lang);
    localStorage.setItem('aext_default_repo', repo);
    if (uiLang !== getLanguage()) setLanguage(uiLang);

    // Sync generator lang selector
    if (elements.selectExtLang) elements.selectExtLang.value = lang;

    if (state.currentUser) loadGithubRepos();

    appendTerminalLog('SUCCESS', t('logs.settings_saved', { lang, repo: repo || '(auto)' }));
    alert(t('alerts.settings_saved'));
  });
}

// ==================== VERSIONES Y DESCARGAS ====================
async function refreshReleaseInfo() {
  try {
    const data = await invokeBackend('check_aext_updates');
    if (data && data.current_version) {
      state.appVersion = data.current_version;
      if (elements.appVersionBadge) elements.appVersionBadge.textContent = `v${data.current_version}`;
      if (elements.settingsCurrentVersion) elements.settingsCurrentVersion.textContent = `v${data.current_version}`;
    }
    renderReleaseTable(data);
    return data;
  } catch (error) {
    renderReleaseTable(null);
    return null;
  }
}

// ==================== CHECK UPDATES ====================
elements.btnCheckUpdates.addEventListener('click', async () => {
  const button = elements.btnCheckUpdates;
  const label = button.querySelector('span');
  if (label) label.textContent = t('rel.checking');
  button.disabled = true;
  try {
    const data = await refreshReleaseInfo();
    if (!data) {
      alert(t('alerts.update_error', { error: 'sin conexión con GitHub' }));
      return;
    }
    if (data.update_available) {
      const open = confirm(t('alerts.update_found', {
        latest: data.latest_version,
        current: data.current_version,
        notes: data.release_notes || '',
      }));
      if (open && data.release_url) {
        await openExternal(data.release_url);
      }
    } else {
      alert(t('alerts.up_to_date', {
        version: data.current_version,
        notes: data.release_notes || '',
      }));
    }
  } catch (e) {
    alert(t('alerts.update_error', { error: e }));
  } finally {
    button.disabled = false;
    if (label) label.textContent = t('rel.check');
  }
});

elements.btnClearLogs.addEventListener('click', () => {
  elements.terminalLogs.innerHTML = '';
});

// ==================== APP INITIALIZATION ====================
window.addEventListener('DOMContentLoaded', async () => {
  appendTerminalLog('INFO', t('logs.init'));
  await checkAuthStatus();
  await loadExtensions();
  refreshBackupExtensions();

  try {
    const status = await invokeBackend('get_system_status');
    if (status && status.app_version) {
      state.appVersion = status.app_version;
      if (elements.appVersionBadge) elements.appVersionBadge.textContent = `v${status.app_version}`;
      if (elements.settingsCurrentVersion) elements.settingsCurrentVersion.textContent = `v${status.app_version}`;
    }
  } catch (_) {
    // El endpoint es opcional (fallback HTTP puede no exponerlo).
  }

  // La tabla de descargas se llena con los assets reales de la última release.
  refreshReleaseInfo();
});
