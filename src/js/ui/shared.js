// Helpers de UI usados por varios paneles (sin dependencias circulares).
import { state, elements, appendTerminalLog, escapeHtml } from '../state.js';
import { invokeBackend } from '../services/tauri.js';
import { t } from '../i18n/index.js';

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

export {
  refreshBackupExtensions,
  loadBackups,
  openExtensionFolder,
};

