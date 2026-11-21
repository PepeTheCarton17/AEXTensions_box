// Modal de resultado de generación (descargas, compilación y enlaces).
import { state, elements, appendTerminalLog, fallbackIcon, iconFor } from '../state.js';
import { saveLocalZip } from '../services/tauri.js';
import { t } from '../i18n/index.js';
import { loadToolchain, compileSuccessExtension } from './build.js';
import { openExtensionFolder } from './shared.js';

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
  if (elements.successActionsStatus) {
    elements.successActionsStatus.classList.add('hidden');
    elements.successActionsStatus.className = 'hidden text-[11px] leading-relaxed';
    elements.successActionsStatus.textContent = '';
    if (ext.github_repo_url && (ext.build_status === 'error' || ext.build_status === 'pending')) {
      const isError = ext.build_status === 'error';
      elements.successActionsStatus.className = `text-[11px] leading-relaxed ${isError ? 'text-rose-300' : 'text-amber-300'}`;
      elements.successActionsStatus.textContent = ext.build_error
        || (isError ? t('ext.build_error') : t('ext.pending'));
    }
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

function initSuccessModal() {
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
}

export {
  showGenerationSuccess,
  closeGenerationSuccess,
  initSuccessModal,
};

