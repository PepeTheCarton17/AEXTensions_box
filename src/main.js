// AEXT Client Engine - Tauri v2 & Embedded API Bridge

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
      return {
        current_version: '1.0.0',
        latest_version: '1.0.0',
        update_available: false,
        release_notes: 'AEXT v1.0.0 al día.'
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

  // AI & MCP Assistance
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

  // Success Modal
  modalGenSuccess: document.getElementById('modal-generation-success'),
  successExtIcon: document.getElementById('success-ext-icon'),
  successExtName: document.getElementById('success-ext-name'),
  successExtPkg: document.getElementById('success-ext-pkg'),
  successExtType: document.getElementById('success-ext-type'),
  successExtVersion: document.getElementById('success-ext-version'),
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

// Descarga un artefacto del servidor local como Blob. Evita que el webview
// navegue a la URL del API (lo que dejaba al usuario en una página sin botón
// de volver) y funciona aunque el archivo sea un ZIP.
async function saveLocalZip(url, filename) {
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
}

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
    elements.authBtnText.textContent = 'Conectar GitHub';
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
      alert(`No se detectó sesión de GitHub CLI: ${e}`);
    }
  });
}

elements.btnSubmitPat.addEventListener('click', async () => {
  const token = elements.inputPatToken.value.trim();
  if (!token) {
    alert('Ingresa un Personal Access Token válido de GitHub');
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
    alert(`Token inválido o expirado: ${e}`);
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
    alert('Por favor ingresa al menos una URL válida (ej. https://animeav1.com)');
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

  const siteIcon = site.favicon_url && site.favicon_url.trim()
    ? site.favicon_url
    : fallbackIcon(site.name);
  const siteIconFallback = fallbackIcon(site.name);

  card.innerHTML = `
    <div class="flex items-start justify-between">
      <div class="flex items-center gap-3">
        <img class="w-8 h-8 rounded-lg bg-surface-800 object-cover" src="${escapeHtml(siteIcon)}" onerror="this.onerror=null;this.src='${siteIconFallback}'">
        <div>
          <h4 class="text-xs font-bold text-slate-100">${escapeHtml(site.name)}</h4>
          <span class="text-[10px] font-mono text-slate-400">${escapeHtml(site.url)}</span>
        </div>
      </div>
      <span class="px-2 py-0.5 rounded text-[10px] font-semibold bg-brand-500/10 text-brand-300 border border-brand-500/20 uppercase">${site.ext_type}</span>
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

    <div class="space-y-1 pt-1 border-t border-surface-800">
      <span class="text-slate-500 block text-[10px]">Servidores detectados:</span>
      <div class="flex flex-wrap gap-1">${hostsBadges}</div>
    </div>

    <div class="space-y-1">
      <span class="text-slate-500 block text-[10px]">Protecciones WAF / Cloudflare:</span>
      <div class="flex flex-wrap gap-1">${protectionsBadges}</div>
    </div>
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
    loadGithubRepos();
  }
}

elements.btnStartGenFlow.addEventListener('click', () => {
  const urls = getEnteredUrls();
  if (urls.length === 0) {
    alert('Ingresa al menos una URL antes de generar la extensión.');
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
    elements.selectExistingRepo.innerHTML = '<option value="">Inicia sesión en GitHub para ver tus repositorios</option>';
    return;
  }
  elements.selectExistingRepo.innerHTML = '<option value="">Cargando repositorios…</option>';
  try {
    const repos = await invokeBackend('list_github_repos') || [];
    state.githubRepos = repos;
    if (repos.length === 0) {
      elements.selectExistingRepo.innerHTML = '<option value="">No se encontraron repositorios</option>';
      return;
    }
    const saved = (localStorage.getItem('aext_default_repo') || '').toLowerCase();
    elements.selectExistingRepo.innerHTML = '<option value="">— Crear un repositorio nuevo —</option>' +
      repos.map(repo => {
        const value = escapeHtml(repo.full_name);
        const selected = repo.full_name.toLowerCase() === saved ? ' selected' : '';
        const visibility = repo.private ? ' (privado)' : '';
        return `<option value="${value}"${selected}>${escapeHtml(repo.full_name)}${visibility}</option>`;
      }).join('');
  } catch (e) {
    console.warn('No se pudieron listar los repositorios:', e);
    elements.selectExistingRepo.innerHTML = '<option value="">No se pudieron cargar los repositorios</option>';
  }
}

if (elements.btnRefreshRepos) {
  elements.btnRefreshRepos.addEventListener('click', loadGithubRepos);
}

// Listener para mostrar/ocultar campos del repo
document.querySelectorAll('input[name="gen-mode"]').forEach(radio => {
  radio.addEventListener('change', (e) => {
    if (e.target.value === 'github') {
      elements.githubRepoExtraFields.classList.remove('hidden');
      // Pre-fill from saved settings if empty
      if (!elements.inputCustomRepoName.value.trim()) {
        const savedRepo = localStorage.getItem('aext_default_repo') || '';
        elements.inputCustomRepoName.value = savedRepo;
      }
      loadGithubRepos();
    } else {
      elements.githubRepoExtraFields.classList.add('hidden');
    }
  });
});

elements.btnConfirmGenMode.addEventListener('click', async () => {
  const urls = getEnteredUrls();
  const selectedMode = document.querySelector('input[name="gen-mode"]:checked').value;
  const selectedRepo = elements.selectExistingRepo?.value?.trim() || '';
  const typedRepo = elements.inputCustomRepoName.value.trim();
  const customRepoName = selectedRepo
    || typedRepo
    || localStorage.getItem('aext_default_repo')?.trim()
    || null;
  const isPrivate = elements.checkPrivateRepo?.checked || false;

  const customName = elements.inputConfirmExtName?.value?.trim() || null;
  const extType = elements.selectConfirmExtType?.value || null;
  const selectedLang = elements.selectConfirmExtLang?.value
    || document.getElementById('select-extension-lang')?.value
    || 'es';

  elements.modalGenMode.classList.add('hidden');
  elements.modalGenMode.classList.remove('flex');

  if (selectedMode === 'github' && !state.currentUser) {
    alert('Has seleccionado "Generar Repositorio en GitHub" pero no has iniciado sesión. Por favor inicia sesión primero con tu cuenta de GitHub o PAT.');
    elements.modalGithubAuth.classList.remove('hidden');
    elements.modalGithubAuth.classList.add('flex');
    return;
  }

  appendTerminalLog('INFO', `Iniciando proceso de generación en modo: ${selectedMode.toUpperCase()}...`);
  appendTerminalLog('INFO', `Nombre: ${customName || guessNameFromInput()}, tipo: ${extType || 'auto'}, idioma: ${selectedLang}`);

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
    appendTerminalLog('SUCCESS', `¡${createdExtensions.length} extensión(es) generada(s) exitosamente!`);

    for (const ext of createdExtensions) {
      if (ext.github_repo_url) {
        appendTerminalLog('SUCCESS', `Repositorio: ${ext.github_repo_url}`);
      }
      if (ext.nekoyomi_repo_url) {
        appendTerminalLog('SUCCESS', `Enlace para Nekoyomi: ${ext.nekoyomi_repo_url}`);
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
    appendTerminalLog('ERROR', `Error en la generación: ${err}`);
    alert(`Ocurrió un error generando la extensión: ${err}`);
  }
});

// ==================== SUCCESS MODAL ====================

function showGenerationSuccess(ext) {
  if (!elements.modalGenSuccess) return;

  elements.successExtIcon.src = iconFor(ext);
  elements.successExtIcon.onerror = () => {
    elements.successExtIcon.onerror = null;
    elements.successExtIcon.src = fallbackIcon(ext.name);
  };
  if (elements.successExtName) elements.successExtName.textContent = ext.name || 'Extensión';
  if (elements.successExtPkg) elements.successExtPkg.textContent = ext.pkg_id || '';
  if (elements.successExtType) elements.successExtType.textContent = (ext.ext_type || 'anime').toUpperCase();
  if (elements.successExtVersion) elements.successExtVersion.textContent = `v${ext.version || '1'}`;

  if (elements.btnSuccessInstallApk) {
    const button = elements.btnSuccessInstallApk;
    button.onclick = null;
    button.removeAttribute('target');
    if (ext.apk_download_url) {
      const isRealApk = /\.apk(\?|$)/i.test(ext.apk_download_url);
      if (isRealApk) {
        button.href = ext.apk_download_url;
        button.setAttribute('download', '');
        button.classList.remove('hidden');
        if (elements.btnSuccessInstallLabel) {
          elements.btnSuccessInstallLabel.textContent = 'Descargar APK';
        }
      } else {
        // ZIP local: descarga por Blob para no navegar fuera de la app.
        button.href = '#';
        button.removeAttribute('download');
        button.onclick = async (event) => {
          event.preventDefault();
          try {
            await saveLocalZip(ext.apk_download_url, `${ext.pkg_id}.zip`);
            appendTerminalLog('SUCCESS', `Código fuente de ${ext.name} descargado (${ext.pkg_id}.zip)`);
          } catch (error) {
            alert(`No se pudo descargar el ZIP:\n${error.message || error}\n\nSi la extensión es antigua, pulsa "Actualizar" en el catálogo para regenerar los archivos.`);
          }
        };
        button.classList.remove('hidden');
        if (elements.btnSuccessInstallLabel) {
          elements.btnSuccessInstallLabel.textContent = 'Descargar código fuente (.zip)';
        }
      }
    } else if (ext.github_repo_url) {
      button.href = `${ext.github_repo_url.replace(/\/$/, '')}/actions`;
      button.removeAttribute('download');
      button.setAttribute('target', '_blank');
      button.classList.remove('hidden');
      if (elements.btnSuccessInstallLabel) {
        elements.btnSuccessInstallLabel.textContent = 'Ver compilación en GitHub Actions';
      }
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
}

function closeGenerationSuccess() {
  elements.modalGenSuccess.classList.add('hidden');
  elements.modalGenSuccess.classList.remove('flex');
  const tabExt = document.querySelector('[data-tab="tab-extensions"]');
  if (tabExt) tabExt.click();
}

if (elements.btnCloseSuccessModal) {
  elements.btnCloseSuccessModal.addEventListener('click', closeGenerationSuccess);
}

if (elements.btnSuccessCopyRepo) {
  elements.btnSuccessCopyRepo.addEventListener('click', async () => {
    const url = elements.btnSuccessCopyRepo.dataset.repoUrl;
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
      alert('¡Enlace copiado! Pégalo en Nekoyomi > Más > Ajustes > Extensiones.');
    } catch (_) {
      alert(`Copia este enlace en Nekoyomi:\n${url}`);
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
        No hay extensiones generadas aún. Ingresa una URL en el Generador para comenzar.
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
        <a href="${escapeHtml(downloadUrl)}" download class="flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-brand-600 hover:bg-brand-500 text-white flex items-center justify-center gap-1.5 transition-all text-center">
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" /></svg>
          <span>Descargar APK</span>
        </a>`;
    } else if (downloadUrl) {
      primaryAction = `
        <button data-download-url="${escapeHtml(downloadUrl)}" data-download-name="${escapeHtml(ext.pkg_id)}.zip" class="btn-download-zip flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-brand-600 hover:bg-brand-500 text-white flex items-center justify-center gap-1.5 transition-all text-center">
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" /></svg>
          <span>Descargar código (.zip)</span>
        </button>`;
    } else if (hasGithub) {
      primaryAction = `
        <a href="${escapeHtml(ext.github_repo_url.replace(/\/$/, ''))}/actions" target="_blank" rel="noreferrer" class="flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-brand-600 hover:bg-brand-500 text-white flex items-center justify-center gap-1.5 transition-all text-center" title="La compilación del APK corre en GitHub Actions">
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" /></svg>
          <span>Ver compilación</span>
        </a>`;
    } else {
      primaryAction = `
        <span class="flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-surface-800 text-slate-500 flex items-center justify-center gap-1.5 text-center">
          Sin descarga disponible
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
          <span class="text-slate-500 block text-[10px]">CMS Detectado</span>
          <span class="text-slate-300 font-medium capitalize">${escapeHtml(ext.cms)}</span>
        </div>
        <div>
          <span class="text-slate-500 block text-[10px]">Audio Preferido</span>
          <span class="text-slate-300 font-medium">${ext.has_sub_dub ? 'Configurable (SUB/DUB)' : 'Estándar'}</span>
        </div>
      </div>

      <!-- Action Buttons -->
      <div class="pt-2 border-t border-surface-800 flex flex-wrap items-center gap-2">
        ${primaryAction}

        <button data-update-pkg="${escapeHtml(ext.pkg_id)}" class="btn-update-ext py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700 flex items-center gap-1.5 transition-all" title="Regenerar y subir una nueva versión">
          <svg class="w-3.5 h-3.5 text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" /></svg>
          <span>Actualizar</span>
        </button>

        ${ext.nekoyomi_repo_url ? `
          <button data-repo-url="${escapeHtml(ext.nekoyomi_repo_url)}" class="btn-copy-repo py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700 flex items-center gap-1.5 transition-all" title="Copiar enlace de repositorio para Nekoyomi">
            <svg class="w-3.5 h-3.5 text-brand-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" /></svg>
            <span>Copiar Repo</span>
          </button>
        ` : ''}

        ${ext.github_repo_url ? `
          <a href="${escapeHtml(ext.github_repo_url)}" target="_blank" class="py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-300 border border-surface-700 flex items-center gap-1 transition-all" title="Ver repositorio en GitHub">
            <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24"><path fill-rule="evenodd" clip-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" /></svg>
          </a>
        ` : ''}

        <!-- Delete button -->
        <button data-delete-pkg="${escapeHtml(ext.pkg_id)}" class="btn-delete-ext p-2 rounded-xl text-slate-500 hover:text-red-400 hover:bg-red-500/10 transition-all" title="Eliminar extensión">
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
        alert('¡Enlace de repositorio copiado al portapapeles! Puedes pegarlo en Nekoyomi > Más > Ajustes > Extensiones.');
      } catch (_) {
        alert(`Copia este enlace en Nekoyomi:\n${url}`);
      }
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
      if (label) label.textContent = 'Descargando…';
      try {
        await saveLocalZip(url, filename);
        appendTerminalLog('SUCCESS', `Descargado ${filename}`);
      } catch (error) {
        appendTerminalLog('ERROR', `No se pudo descargar ${filename}: ${error.message || error}`);
        alert(`No se pudo descargar el ZIP:\n${error.message || error}\n\nSi la extensión se creó con una versión anterior de AEXT, pulsa "Actualizar" para regenerar los archivos.`);
      } finally {
        button.disabled = false;
        button.classList.remove('opacity-60', 'cursor-wait');
        if (label) label.textContent = original || 'Descargar código (.zip)';
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
      if (label) label.textContent = 'Actualizando…';
      appendTerminalLog('INFO', `Actualizando ${pkgId}...`);
      try {
        const updated = await invokeBackend('update_extension', { pkgId });
        appendTerminalLog('SUCCESS', `Extensión ${updated ? updated.name : pkgId} actualizada a v${updated ? updated.version : '?'}`);
        await loadExtensions();
      } catch (err) {
        appendTerminalLog('ERROR', `No se pudo actualizar ${pkgId}: ${err}`);
        alert(`No se pudo actualizar la extensión:\n${err}`);
        button.disabled = false;
        button.classList.remove('opacity-60', 'cursor-wait');
        if (label) label.textContent = original || 'Actualizar';
      }
    });
  });

  // Hook delete buttons
  document.querySelectorAll('.btn-delete-ext').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const pkgId = e.currentTarget.getAttribute('data-delete-pkg');
      if (confirm(`¿Estás seguro de eliminar la extensión ${pkgId}?`)) {
        await invokeBackend('delete_extension', { pkgId });
        loadExtensions();
      }
    });
  });
}

elements.btnRefreshExtensions.addEventListener('click', loadExtensions);

// ==================== STREAM EXTRACTOR TESTER & DIAGNOSTICS ====================
elements.btnTestStream.addEventListener('click', async () => {
  const url = elements.testStreamInput.value.trim();
  if (!url) {
    alert('Ingresa una URL de stream o embed para probar');
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

// ==================== AI ASSISTANCE & MCP EVENTS ====================
if (elements.btnCopyMcpConfig) {
  elements.btnCopyMcpConfig.addEventListener('click', () => {
    const config = {
      mcpServers: {
        aext: {
          url: "http://localhost:45731/mcp"
        }
      }
    };
    navigator.clipboard.writeText(JSON.stringify(config, null, 2));
    alert('¡Configuración MCP copiada al portapapeles!\nPégala en tu cliente de IA (Claude Desktop, Antigravity o Cursor) en la sección mcpServers.');
  });
}

if (elements.btnCopyLogsAi) {
  elements.btnCopyLogsAi.addEventListener('click', async () => {
    try {
      const logs = await invokeBackend('get_system_logs') || [];
      const text = logs.map(l => `[${l.timestamp}] [${l.level}] ${l.message}`).join('\n');
      navigator.clipboard.writeText(`LOGS AEXT:\n${text}`);
      alert('¡Logs de terminal copiados para compartir con tu IA!');
    } catch (e) {
      alert('Error copiando logs');
    }
  });
}

if (elements.btnGenAiDiag) {
  elements.btnGenAiDiag.addEventListener('click', async () => {
    try {
      const res = await fetch('http://localhost:45731/api/ai/diagnostics');
      const diag = await res.json();
      navigator.clipboard.writeText(JSON.stringify(diag, null, 2));
      alert('¡Reporte completo de diagnóstico generado y copiado al portapapeles en JSON!\nPuedes enviárselo directamente a tu IA para que diagnostique y repare la extensión.');
    } catch (e) {
      alert('No se pudo generar el reporte de diagnóstico del servidor API embebido');
    }
  });
}

if (elements.btnApplyAiPatch) {
  elements.btnApplyAiPatch.addEventListener('click', async () => {
    const val = elements.inputAiPatch.value.trim();
    if (!val) {
      alert('Ingresa el JSON del parche de IA');
      return;
    }
    try {
      const patch = JSON.parse(val);
      const res = await fetch('http://localhost:45731/api/ai/patch', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(patch)
      });
      const data = await res.json();
      if (res.ok) {
        appendTerminalLog('SUCCESS', `Parche de IA aplicado con éxito a ${patch.pkg_id} (Nueva versión: ${data.version})`);
        alert(`¡Parche de IA aplicado exitosamente!\nExtensión actualizada a la versión ${data.version}`);
        loadExtensions();
      } else {
        alert(`Error aplicando parche: ${data.error}`);
      }
    } catch (e) {
      alert(`JSON inválido: ${e.message}`);
    }
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

    localStorage.setItem('aext_lang', lang);
    localStorage.setItem('aext_default_repo', repo);

    // Sync generator lang selector
    if (elements.selectExtLang) elements.selectExtLang.value = lang;

    if (state.currentUser) loadGithubRepos();

    appendTerminalLog('SUCCESS', `Ajustes guardados: idioma=${lang}, repo=${repo || '(auto-generar)'}`);
    alert('Ajustes guardados correctamente.');
  });
}

// ==================== CHECK UPDATES ====================
elements.btnCheckUpdates.addEventListener('click', async () => {
  const button = elements.btnCheckUpdates;
  const label = button.querySelector('span');
  const original = label ? label.textContent : '';
  if (label) label.textContent = 'Buscando…';
  button.disabled = true;
  try {
    const data = await invokeBackend('check_aext_updates');
    if (data.update_available) {
      const open = confirm(
        `Nueva versión disponible: v${data.latest_version}\n` +
        `Tienes instalada la v${data.current_version}.\n\n` +
        `${data.release_notes || ''}\n\n¿Abrir la página de descarga?`
      );
      if (open && data.release_url) {
        window.open(data.release_url, '_blank');
      }
    } else {
      alert(`AEXT v${data.current_version}\n\n${data.release_notes || 'Estás ejecutando la versión más reciente.'}`);
    }
  } catch (e) {
    alert(`Error al verificar actualizaciones: ${e}`);
  } finally {
    button.disabled = false;
    if (label) label.textContent = original || 'Buscar Actualizaciones';
  }
});

elements.btnClearLogs.addEventListener('click', () => {
  elements.terminalLogs.innerHTML = '';
});

// ==================== APP INITIALIZATION ====================
window.addEventListener('DOMContentLoaded', async () => {
  appendTerminalLog('INFO', 'AEXT inicializado. Comprobando estado de GitHub y servidor API...');
  await checkAuthStatus();
  await loadExtensions();

  try {
    const status = await invokeBackend('get_system_status');
    if (status && elements.appVersionBadge) {
      elements.appVersionBadge.textContent = `v${status.app_version}`;
    }
  } catch (_) {
    // El endpoint es opcional (fallback HTTP puede no exponerlo).
  }
});
