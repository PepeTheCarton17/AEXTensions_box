// Compilación local de APK y estado del toolchain.
import { state, elements, appendTerminalLog } from '../state.js';
import { invokeBackend } from '../services/tauri.js';
import { t } from '../i18n/index.js';
import { openExtensionFolder } from './shared.js';
import { loadExtensions, renderExtensionsGrid } from './catalog.js';

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

function markExtensionBuild(pkgId, status, extra = {}) {
  const item = state.extensions.find(e => e.pkg_id === pkgId || e.id === pkgId);
  if (item) {
    item.build_status = status;
    item.build_error = extra.error || null;
    item.build_apk_path = extra.apk_path || null;
  }
  renderExtensionsGrid(state.extensions);
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

  // Estado optimista: aunque el usuario cambie de pestaña, la tarjeta ya
  // muestra "compilando" y el backend persistirá el resultado al terminar.
  markExtensionBuild(ext.pkg_id, 'building');

  try {
    const result = await invokeBackend('compile_extension', { pkgId: ext.pkg_id });
    if (elements.successCompileOutput) {
      elements.successCompileOutput.textContent = result.output || '';
      elements.successCompileOutput.classList.remove('hidden');
    }
    if (result.success) {
      markExtensionBuild(ext.pkg_id, 'success', { apk_path: result.apk_path });
      appendTerminalLog('SUCCESS', t('logs.compile_ok', { path: result.apk_path || '' }));
      if (elements.successDownloadStatus) {
        elements.successDownloadStatus.classList.remove('hidden');
        elements.successDownloadStatus.textContent = `APK: ${result.apk_path}`;
      }
      await loadExtensions();
      await openExtensionFolder(ext.pkg_id);
    } else {
      const detail = result.error || t('logs.compile_fail', { pkg: ext.pkg_id });
      markExtensionBuild(ext.pkg_id, 'error', { error: detail });
      appendTerminalLog(
        'ERROR',
        `${t('logs.compile_fail', { pkg: ext.pkg_id })}\n${t('logs.compile_detail', { detail })}`
      );
      alert(`${t('alerts.compile_failed')}\n\n${detail}`);
      await loadExtensions();
    }
  } catch (error) {
    const detail = String(error);
    markExtensionBuild(ext.pkg_id, 'error', { error: detail });
    if (elements.successCompileOutput) {
      elements.successCompileOutput.classList.remove('hidden');
      elements.successCompileOutput.textContent = detail;
    }
    appendTerminalLog('ERROR', detail);
    alert(detail);
    await loadExtensions();
  } finally {
    button.disabled = false;
    button.classList.remove('opacity-60', 'cursor-wait');
    if (label) label.textContent = original || t('succ.compile');
  }
}

export {
  loadToolchain,
  renderToolchainStatus,
  markExtensionBuild,
  compileSuccessExtension,
};

