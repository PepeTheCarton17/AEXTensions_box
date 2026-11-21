// Flujo de creación de extensiones y modal de selección de destino.
import { state, elements, appendTerminalLog, escapeHtml, REPO_NONE_VALUE } from '../state.js';
import { invokeBackend } from '../services/tauri.js';
import { t } from '../i18n/index.js';
import { getEnteredUrls } from './analysis.js';
import { showGenerationSuccess } from './modals.js';

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

async function loadGithubRepos() {
  if (!elements.selectExistingRepo) return;
  if (!state.currentUser) {
    elements.selectExistingRepo.innerHTML = `<option value="${REPO_NONE_VALUE}">${escapeHtml(t('modal.repo_none'))}</option>`;
    syncRepoSelection();
    return;
  }
  elements.selectExistingRepo.innerHTML = `<option value="">${escapeHtml(t('modal.repo_loading'))}</option>`;
  try {
    const repos = await invokeBackend('list_github_repos') || [];
    state.githubRepos = repos;
    if (repos.length === 0) {
      elements.selectExistingRepo.innerHTML = `<option value="${REPO_NONE_VALUE}">${escapeHtml(t('modal.repo_none'))}</option>`;
      syncRepoSelection();
      return;
    }
    const saved = (localStorage.getItem('aext_default_repo') || '').toLowerCase();
    elements.selectExistingRepo.innerHTML = `<option value="${REPO_NONE_VALUE}">${escapeHtml(t('modal.repo_none'))}</option>` +
      repos.map(repo => {
        const value = escapeHtml(repo.full_name);
        const selected = repo.full_name.toLowerCase() === saved ? ' selected' : '';
        const visibility = repo.private ? ' (privado)' : '';
        return `<option value="${value}"${selected}>${escapeHtml(repo.full_name)}${visibility}</option>`;
      }).join('');
    syncRepoSelection();
  } catch (e) {
    console.warn('No se pudieron listar los repositorios:', e);
    elements.selectExistingRepo.innerHTML = `<option value="${REPO_NONE_VALUE}">${escapeHtml(t('modal.repo_none'))}</option>`;
    syncRepoSelection();
  }
}

/// Si hay un repositorio seleccionado, el campo "otro repo" queda desactivado
/// para que no haya dos destinos contradictorios. Con «Ninguno» se habilita.
function syncRepoSelection() {
  if (!elements.selectExistingRepo || !elements.inputCustomRepoName) return;
  const selected = elements.selectExistingRepo.value.trim();
  const input = elements.inputCustomRepoName;
  if (selected && selected !== REPO_NONE_VALUE) {
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

function initGenerationFlow() {
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
    const selectedRepoRaw = elements.selectExistingRepo?.value?.trim() || '';
    const typedRepo = elements.inputCustomRepoName?.value?.trim() || '';
    const explicitNone = selectedRepoRaw === REPO_NONE_VALUE;
    const selectedRepo = explicitNone ? '' : selectedRepoRaw;
    // Si no se eligió ni se tecleó nada, aplica el repo predeterminado guardado.
    // Elegir «Ninguno» explícitamente sí fuerza la creación de uno nuevo.
    const savedDefaultRepo = (localStorage.getItem('aext_default_repo') || '').trim();
    const customRepoName = selectedRepo || typedRepo || (!explicitNone ? savedDefaultRepo : '') || null;
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
      const actionsFailed = createdExtensions.filter(ext => ext.build_status === 'error').length;
      const actionsPending = createdExtensions.filter(
        ext => ext.build_status === 'pending' || ext.build_status === 'building'
      ).length;
      if (actionsFailed > 0) {
        appendTerminalLog('ERROR', t('logs.gen_actions_failed', { count: actionsFailed }));
      } else if (actionsPending > 0) {
        appendTerminalLog('WARN', t('logs.gen_actions_pending', { count: actionsPending }));
      } else {
        appendTerminalLog('SUCCESS', t('logs.gen_success', { count: createdExtensions.length }));
      }

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
}

export {
  guessNameFromInput,
  openGenerationModal,
  loadGithubRepos,
  syncRepoSelection,
  initGenerationFlow,
};

