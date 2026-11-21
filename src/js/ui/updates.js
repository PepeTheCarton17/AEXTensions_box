// Aviso de actualización de extensiones tras instalar una nueva versión.
import { state, elements, appendTerminalLog, UPDATE_PROMPT_KEY, isRepoMissing } from '../state.js';
import { invokeBackend } from '../services/tauri.js';
import { t } from '../i18n/index.js';
import { refreshRepoFilters, loadExtensions } from './catalog.js';
import { refreshBackupExtensions } from './shared.js';

function extensionsForRepo(repoUrl) {
  // El backend excluye las fichas marcadas como eliminadas. El conteo de la UI
  // debe usar exactamente el mismo criterio o anunciará más extensiones de las
  // que realmente se van a actualizar.
  if (!repoUrl) return state.extensions.filter((ext) => !isRepoMissing(ext));
  const selected = repoUrl.replace(/\/$/, '');
  return state.extensions.filter(
    (ext) => !isRepoMissing(ext) && (ext.github_repo_url || '').replace(/\/$/, '') === selected
  );
}

function setBulkBusy(busy) {
  for (const btn of [
    elements.btnSyncExtensions,
    elements.btnUpdateAll,
    elements.btnUpdateRepo,
    elements.btnUpdatePromptAll,
    elements.btnUpdatePromptRepo,
  ]) {
    if (!btn) continue;
    btn.disabled = busy;
    btn.classList.toggle('opacity-60', busy);
    btn.classList.toggle('cursor-wait', busy);
  }
}

function closeUpdatePrompt(markSeen = true) {
  if (!elements.updatePromptModal) return;
  if (markSeen && state.appVersion) {
    try {
      localStorage.setItem(UPDATE_PROMPT_KEY, state.appVersion);
    } catch (_) {
      // localStorage puede estar bloqueado; el aviso simplemente se repetirá.
    }
  }
  elements.updatePromptModal.classList.add('hidden');
  elements.updatePromptModal.classList.remove('flex');
}

function refreshUpdatePromptCount() {
  if (!elements.updatePromptCount) return;
  const repo = elements.updatePromptRepo?.value || '';
  const selected = repo.replace(/\/$/, '');
  const detected = repo
    ? state.extensions.filter(
        (ext) => (ext.github_repo_url || '').replace(/\/$/, '') === selected
      )
    : state.extensions;
  const targets = extensionsForRepo(repo);
  const skipped = detected.length - targets.length;
  elements.updatePromptCount.textContent = [
    t('ext.bulk_detected', { count: detected.length }),
    t('ext.bulk_updating', { count: targets.length }),
    skipped > 0 ? t('ext.bulk_skipped', { count: skipped }) : '',
  ].filter(Boolean).join(' ');
}

function openUpdatePrompt() {
  if (!elements.updatePromptModal) return;
  refreshRepoFilters();
  refreshUpdatePromptCount();
  elements.updatePromptModal.classList.remove('hidden');
  elements.updatePromptModal.classList.add('flex');
}

async function runBulkUpdate(repoUrl = '') {
  const targets = extensionsForRepo(repoUrl);
  if (targets.length === 0) {
    alert(t('ext.select_repo'));
    return;
  }
  const selected = repoUrl.replace(/\/$/, '');
  const detected = repoUrl
    ? state.extensions.filter(
        (ext) => (ext.github_repo_url || '').replace(/\/$/, '') === selected
      )
    : state.extensions;
  setBulkBusy(true);
  appendTerminalLog('INFO', t('ext.bulk_detected', { count: detected.length }));
  appendTerminalLog('INFO', t('ext.bulk_updating', { count: targets.length }));
  const skipped = detected.length - targets.length;
  if (skipped > 0) {
    appendTerminalLog('WARN', t('ext.bulk_skipped', { count: skipped }));
  }
  try {
    const updated = await invokeBackend('update_all_extensions', {
      repoUrl: repoUrl ? repoUrl.replace(/\/$/, '') : null,
      lang: localStorage.getItem('aext_lang') || 'es',
    });
    appendTerminalLog('SUCCESS', t('ext.bulk_ok', { count: updated ? updated.length : 0 }));
    closeUpdatePrompt();
    await loadExtensions();
    refreshBackupExtensions();
  } catch (err) {
    const detail = String(err);
    appendTerminalLog('ERROR', detail);
    alert(detail);
  } finally {
    setBulkBusy(false);
  }
}

function maybeShowUpdatePrompt() {
  if (!state.appVersion || state.appVersion === 'dev' || !elements.updatePromptModal) return;
  let seen = null;
  try {
    seen = localStorage.getItem(UPDATE_PROMPT_KEY);
  } catch (_) {
    // Sin localStorage no podemos recordar la versión; mostramos el aviso.
  }
  if (seen === state.appVersion) return;
  if (state.extensions.length === 0) {
    closeUpdatePrompt();
    return;
  }
  openUpdatePrompt();
}

export {
  extensionsForRepo,
  setBulkBusy,
  closeUpdatePrompt,
  refreshUpdatePromptCount,
  openUpdatePrompt,
  runBulkUpdate,
  maybeShowUpdatePrompt,
};

