// Acciones del catálogo: verificación de repo, búsqueda y actualizaciones.
import { state, elements, appendTerminalLog } from '../state.js';
import { invokeBackend } from '../services/tauri.js';
import { t } from '../i18n/index.js';
import { refreshRepoFilters, loadExtensions, renderExtensionsGrid } from './catalog.js';
import { runBulkUpdate, closeUpdatePrompt, refreshUpdatePromptCount, setBulkBusy } from './updates.js';

async function checkRepoState() {
  if (!elements.btnCheckRepo) return 0;
  const button = elements.btnCheckRepo;
  button.disabled = true;
  button.classList.add('opacity-60', 'cursor-wait');
  try {
    const result = await invokeBackend('check_extensions_in_repo');
    if (result && result.missing > 0) {
      appendTerminalLog('WARN', t('logs.repo_missing', { count: result.missing }));
    } else if (result) {
      appendTerminalLog('SUCCESS', t('logs.repo_ok', { count: result.checked || 0 }));
    }
    return result ? result.missing || 0 : 0;
  } catch (err) {
    // Sin sesión de GitHub o sin red no podemos afirmar nada: se mantienen los
    // estados anteriores y se muestra el motivo.
    appendTerminalLog('WARN', t('logs.repo_check_error', { error: String(err) }));
    return 0;
  } finally {
    button.disabled = false;
    button.classList.remove('opacity-60', 'cursor-wait');
  }
}

function initExtensionActions() {
  elements.btnRefreshExtensions.addEventListener('click', async () => {
    await checkRepoState();
    await loadExtensions();
  });
  elements.btnCheckRepo?.addEventListener('click', async () => {
    await checkRepoState();
    await loadExtensions();
  });
  elements.extSearch?.addEventListener('input', () => {
    elements.badgeExtCount.textContent = filteredExtensions().length;
    renderExtensionsGrid();
  });

  elements.extRepoFilter?.addEventListener('change', () => {
    elements.badgeExtCount.textContent = filteredExtensions().length;
    renderExtensionsGrid();
  });

  elements.btnSyncExtensions?.addEventListener('click', async () => {
    setBulkBusy(true);
    try {
      const adopted = await invokeBackend('sync_extension_catalog');
      appendTerminalLog('SUCCESS', t('ext.sync_ok', { count: adopted || 0 }));
      await loadExtensions();
      refreshBackupExtensions();
    } catch (err) {
      const detail = String(err);
      appendTerminalLog('ERROR', detail);
      alert(detail);
    } finally {
      setBulkBusy(false);
    }
  });

  elements.btnUpdateAll?.addEventListener('click', () => runBulkUpdate(''));
  elements.btnUpdateRepo?.addEventListener('click', () => {
    const repo = elements.extRepoFilter?.value || '';
    if (!repo) {
      alert(t('ext.select_repo'));
      return;
    }
    runBulkUpdate(repo);
  });

  elements.updatePromptRepo?.addEventListener('change', refreshUpdatePromptCount);
  elements.btnUpdatePromptAll?.addEventListener('click', () => runBulkUpdate(''));
  elements.btnUpdatePromptRepo?.addEventListener('click', () => {
    const repo = elements.updatePromptRepo?.value || '';
    if (!repo) {
      alert(t('ext.select_repo'));
      return;
    }
    runBulkUpdate(repo);
  });
  elements.btnUpdatePromptLater?.addEventListener('click', () => closeUpdatePrompt());
}

export {
  checkRepoState,
  initExtensionActions,
};

