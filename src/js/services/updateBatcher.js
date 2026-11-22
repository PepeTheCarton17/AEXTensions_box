// Ventana de agrupación de actualizaciones manuales. La primera pulsación
// espera WINDOW_MS para que el usuario pueda pulsar otras tarjetas; todas se
// envían en un único comando y, por tanto, en un único commit por repo.
import { appendTerminalLog } from '../state.js';
import { invokeBackend } from './tauri.js';
import { t } from '../i18n/index.js';
import { setBulkBusy } from './updateBusy.js';

const WINDOW_MS = 10_000;
const COOLDOWN_MS = 1_500;
const pending = new Map();
let flushTimer = null;
let running = false;

function secondsSince(startedAt) {
  return ((Date.now() - startedAt) / 1000).toFixed(1);
}

function logUpdateResult(item, startedAt) {
  if (!item) return;
  const vars = {
    pkg: item.pkg_id || '?',
    version: item.version || '?',
    status: item.build_status || 'pending',
    error: item.build_error || t('ext.build_error'),
    seconds: secondsSince(startedAt),
    url: item.actions_run_url || '',
  };
  if (vars.status === 'error') {
    appendTerminalLog('ERROR', t('logs.update_result_error', vars));
  } else if (vars.status === 'success') {
    appendTerminalLog('SUCCESS', t('logs.update_result_ok', vars));
  } else {
    appendTerminalLog('WARN', t('logs.update_result_pending', vars));
  }
}

function logUpdateResults(items, startedAt) {
  const list = Array.isArray(items) ? items : [];
  for (const item of list) logUpdateResult(item, startedAt);
}

function queueManualUpdate(pkgId) {
  if (!pkgId || pending.has(pkgId)) return;
  const lang = localStorage.getItem('aext_lang') || 'es';
  pending.set(pkgId, { pkgId, lang });
  // Durante la ventana NO se bloquean las demás tarjetas: el usuario debe
  // poder pulsar varias. El bloqueo global solo empieza al ejecutar el lote.
  appendTerminalLog('INFO', t('logs.update_queued', {
    pkg: pkgId,
    count: pending.size,
    seconds: WINDOW_MS / 1000,
  }));
  if (flushTimer === null) {
    flushTimer = setTimeout(() => {
      flushTimer = null;
      flush();
    }, WINDOW_MS);
  }
}

async function refreshCatalog() {
  // Importación dinámica: catalog.js importa este módulo para encolar clics.
  const { loadExtensions } = await import('../ui/catalog.js');
  const { refreshBackupExtensions } = await import('../ui/shared.js');
  await loadExtensions();
  refreshBackupExtensions();
}

async function flush() {
  if (running || pending.size === 0) return;
  running = true;
  const batch = [...pending.values()];
  pending.clear();
  const startedAt = Date.now();
  setBulkBusy(true);
  appendTerminalLog('INFO', t('logs.update_batch_start', { count: batch.length }));
  try {
    const updated = await invokeBackend('update_extensions_batch', {
      pkgIds: batch.map((entry) => entry.pkgId),
      lang: batch[0].lang,
    });
    logUpdateResults(updated, startedAt);
    appendTerminalLog('SUCCESS', t('logs.update_batch_done', {
      count: Array.isArray(updated) ? updated.length : 0,
      seconds: secondsSince(startedAt),
    }));
    await refreshCatalog();
  } catch (err) {
    const detail = String(err?.message || err);
    appendTerminalLog('ERROR', t('logs.update_batch_error', { error: detail }));
    alert(t('logs.update_batch_error', { error: detail }));
  } finally {
    running = false;
    if (pending.size > 0) {
      // Pausa breve antes de abrir la siguiente ventana: evita encadenar
      // escrituras en GitHub y deja que Actions indexe el run anterior.
      appendTerminalLog('INFO', t('logs.update_batch_cooldown', {
        seconds: COOLDOWN_MS / 1000,
      }));
      setTimeout(() => {
        if (pending.size > 0 && !running) {
          flushTimer = setTimeout(() => {
            flushTimer = null;
            flush();
          }, WINDOW_MS);
        } else {
          setBulkBusy(false);
        }
      }, COOLDOWN_MS);
    } else {
      setBulkBusy(false);
    }
  }
}

function hasQueuedUpdates() {
  return pending.size > 0;
}

export { queueManualUpdate, hasQueuedUpdates, logUpdateResult, logUpdateResults };
