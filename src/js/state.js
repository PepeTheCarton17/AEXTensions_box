// Estado compartido, elementos del DOM y utilidades puras de AEXT.
// «Ninguno» es una opción explícita (crear repo nuevo), distinta de «todavía
// no se eligió nada» (donde aplica el repo predeterminado de los ajustes).
const REPO_NONE_VALUE = '__none__';

// Se guarda la versión exacta de AEXT que ya vio el aviso. Al instalar otra
// versión, la clave no coincide y el aviso vuelve a aparecer una sola vez.
const UPDATE_PROMPT_KEY = 'aext.update_prompt_version';

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
  deletedExtensionsSection: document.getElementById('deleted-extensions-section'),
  deletedExtensionsGrid: document.getElementById('deleted-extensions-grid'),
  deletedExtCount: document.getElementById('deleted-ext-count'),
  badgeExtCount: document.getElementById('badge-ext-count'),
  btnRefreshExtensions: document.getElementById('btn-refresh-extensions'),
  btnCheckRepo: document.getElementById('btn-check-repo'),
  extSearch: document.getElementById('ext-search'),
  extRepoFilter: document.getElementById('ext-repo-filter'),
  btnSyncExtensions: document.getElementById('btn-sync-extensions'),
  btnUpdateAll: document.getElementById('btn-update-all'),
  btnUpdateRepo: document.getElementById('btn-update-repo'),
  updatePromptModal: document.getElementById('update-prompt-modal'),
  updatePromptRepo: document.getElementById('update-prompt-repo'),
  updatePromptCount: document.getElementById('update-prompt-count'),
  btnUpdatePromptAll: document.getElementById('btn-update-prompt-all'),
  btnUpdatePromptRepo: document.getElementById('btn-update-prompt-repo'),
  btnUpdatePromptLater: document.getElementById('btn-update-prompt-later'),

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
  successActionsStatus: document.getElementById('success-actions-status'),
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
    <span class="${colorClass} flex-1 break-all whitespace-pre-wrap">${escapeHtml(message)}</span>
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
  const embedded = ext && typeof ext.icon_base64 === 'string' ? ext.icon_base64.trim() : '';
  if (embedded) return embedded;
  // Las variantes adoptadas desde el índice publicado no tienen icono incrustado:
  // su PNG vive en gh-pages/icon/<pkg>.png.
  const published = ext && typeof ext.icon_url === 'string' ? ext.icon_url.trim() : '';
  return published || fallbackIcon(ext && ext.name);
}

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

function isRepoMissing(ext) {
  return ext.repo_state === 'missing' && !ext.repo_ignore_missing;
}

export {
  REPO_NONE_VALUE,
  UPDATE_PROMPT_KEY,
  state,
  elements,
  appendTerminalLog,
  escapeHtml,
  fallbackIcon,
  iconFor,
  formatBytes,
  isRepoMissing,
};

