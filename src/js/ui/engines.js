// Registro de motores instalables: instalar desde repo, activar/desactivar,
// probar con peticiones anónimas y mostrar resultados verificados.
import { state, elements, appendTerminalLog, escapeHtml } from '../state.js';
import { invokeBackend } from '../services/tauri.js';
import { t } from '../i18n/index.js';

state.engines = [];
state.engineTrials = [];

function kindLabel(engine) {
  const key = `tools.engines_kind_${engine.kind || 'extractor'}`;
  return t(key);
}

export function renderEngines() {
  if (!elements.enginesList) return;
  const engines = state.engines || [];
  if (!engines.length) {
    elements.enginesList.innerHTML = `<p class="text-xs text-slate-500">${escapeHtml(t('tools.engines_empty'))}</p>`;
    return;
  }
  elements.enginesList.innerHTML = engines.map((engine) => {
    const builtin = engine.source === 'builtin';
    const enabled = engine.enabled !== false;
    const id = escapeHtml(engine.id);
    const toggleLabel = escapeHtml(t(enabled ? 'tools.engines_disable' : 'tools.engines_enable'));
    const uninstallLabel = escapeHtml(t('tools.engines_uninstall'));
    return `
      <div class="p-3 bg-surface-950 rounded-xl border border-surface-800 space-y-1">
        <div class="flex items-center justify-between gap-2">
          <div class="flex items-center gap-2 flex-wrap">
            <span class="text-xs font-semibold text-slate-200">${escapeHtml(engine.name || engine.id)}</span>
            <span class="text-[10px] px-1.5 py-0.5 rounded bg-indigo-500/10 text-indigo-300 border border-indigo-500/20">${escapeHtml(kindLabel(engine))}</span>
            <span class="text-[10px] px-1.5 py-0.5 rounded ${enabled ? 'bg-emerald-500/10 text-emerald-300' : 'bg-slate-700 text-slate-400'}">${toggleLabel}</span>
          </div>
          <div class="flex items-center gap-1">
            <button data-engine-action="toggle" data-engine-id="${id}" data-enabled="${enabled}" class="px-2 py-1 rounded-lg text-[10px] bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700">${toggleLabel}</button>
            ${builtin ? '' : `<button data-engine-action="uninstall" data-engine-id="${id}" class="px-2 py-1 rounded-lg text-[10px] bg-rose-500/10 hover:bg-rose-500/20 text-rose-300 border border-rose-500/20">${uninstallLabel}</button>`}
          </div>
        </div>
        <div class="text-[10px] text-slate-500 break-all">${id} · ${escapeHtml(engine.license || '—')} · ${escapeHtml(engine.provenance || '—')}</div>
        <div class="text-[10px] text-slate-400 break-all">${escapeHtml((engine.capabilities || []).join(', '))}</div>
      </div>`;
  }).join('');
}

export function renderEngineTrials() {
  if (!elements.enginesTestResult) return;
  const trials = state.engineTrials || [];
  if (!trials.length) {
    elements.enginesTestResult.innerHTML = '';
    return;
  }
  elements.enginesTestResult.innerHTML = trials.map((trial) => {
    const ok = Boolean(trial.success);
    const label = escapeHtml(t(ok ? 'tools.engines_result_ok' : 'tools.engines_result_fail'));
    const color = ok ? 'text-emerald-300' : 'text-rose-300';
    return `<div class="text-[11px] ${color} break-all">
      [${label}] ${escapeHtml(trial.engine_name || trial.engine_id)} · ${escapeHtml(trial.probe_id)} · ${escapeHtml(trial.detail)}
      <span class="text-slate-500">${escapeHtml(trial.url)}</span>
    </div>`;
  }).join('');
}

async function loadEngines() {
  try {
    state.engines = await invokeBackend('list_engines') || [];
    renderEngines();
  } catch (error) {
    appendTerminalLog('ERROR', t('logs.engine_list_error', { error: error.message || error }));
  }
}

async function installEngine() {
  const repo = (elements.inputEngineRepo && elements.inputEngineRepo.value || '').trim();
  if (!repo) {
    alert(t('alerts.engine_repo_required'));
    return;
  }
  try {
    const engine = await invokeBackend('install_engine', { repoUrl: repo });
    appendTerminalLog('SUCCESS', t('logs.engine_installed', { name: engine.name, license: engine.license }));
    elements.inputEngineRepo.value = '';
    await loadEngines();
  } catch (error) {
    appendTerminalLog('ERROR', t('logs.engine_install_error', { error: error.message || error }));
  }
}

async function toggleEngine(id, enabled) {
  try {
    state.engines = await invokeBackend('set_engine_enabled', { id, enabled }) || [];
    appendTerminalLog('INFO', t('logs.engine_toggle', { id, state: enabled ? t('tools.engines_enable') : t('tools.engines_disable') }));
    renderEngines();
  } catch (error) {
    appendTerminalLog('ERROR', t('logs.engine_install_error', { error: error.message || error }));
  }
}

async function uninstallEngine(id) {
  try {
    state.engines = await invokeBackend('uninstall_engine', { id }) || [];
    appendTerminalLog('WARN', t('logs.engine_uninstalled', { id }));
    renderEngines();
  } catch (error) {
    appendTerminalLog('ERROR', t('logs.engine_install_error', { error: error.message || error }));
  }
}

async function testEngines() {
  const url = (elements.inputEngineTestUrl && elements.inputEngineTestUrl.value || '').trim();
  if (!url) {
    alert(t('alerts.engine_url_required'));
    return;
  }
  if (elements.enginesTestResult) {
    elements.enginesTestResult.innerHTML = `<span class="text-xs text-slate-400">${escapeHtml(t('tools.engines_testing'))}</span>`;
  }
  try {
    state.engineTrials = await invokeBackend('test_engines', { url }) || [];
    renderEngineTrials();
    const ok = state.engineTrials.filter((trial) => trial.success).length;
    appendTerminalLog(ok ? 'SUCCESS' : 'WARN', t('logs.engine_test_done', { count: ok, total: state.engineTrials.length }));
  } catch (error) {
    state.engineTrials = [];
    renderEngineTrials();
    appendTerminalLog('ERROR', t('logs.engine_test_error', { error: error.message || error }));
  }
}

export function initEngines() {
  if (elements.btnInstallEngine) elements.btnInstallEngine.addEventListener('click', installEngine);
  if (elements.btnTestEngines) elements.btnTestEngines.addEventListener('click', testEngines);
  if (elements.enginesList) {
    elements.enginesList.addEventListener('click', (event) => {
      const button = event.target.closest('button[data-engine-action]');
      if (!button) return;
      const id = button.getAttribute('data-engine-id');
      if (button.getAttribute('data-engine-action') === 'toggle') {
        toggleEngine(id, button.getAttribute('data-enabled') !== 'true');
      } else {
        uninstallEngine(id);
      }
    });
  }
  loadEngines();
}
