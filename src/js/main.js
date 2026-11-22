// AEXT Client Engine - Tauri v2 & Embedded API Bridge (bootstrap)
// Orquesta los módulos respetando el orden original de registro de listeners.

import { initLanguage, t } from './i18n/index.js';
import { state, elements, appendTerminalLog } from './state.js';
import { initExternalLinks, invokeBackend } from './services/tauri.js';
import { initAuth, checkAuthStatus } from './ui/auth.js';
import { initAnalysis, renderAnalysisCard } from './ui/analysis.js';
import { initGenerationFlow, syncRepoSelection } from './ui/generator.js';
import { initSuccessModal } from './ui/modals.js';
import { loadExtensions, renderExtensionsGrid } from './ui/catalog.js';
import { initExtensionActions, checkRepoState } from './ui/extensions.js';
import { renderToolchainStatus } from './ui/build.js';
import { initStreamTester, initTools } from './ui/tools.js';
import { initEngines, renderEngines, renderEngineTrials } from './ui/engines.js';
import { initSettings, initLogControls } from './ui/settings.js';
import { renderReleaseTable, refreshReleaseInfo, initReleases } from './ui/releases.js';
import { refreshBackupExtensions, loadBackups } from './ui/shared.js';
import { maybeShowUpdatePrompt } from './ui/updates.js';

// Re-renderiza el contenido dinámico al cambiar el idioma.
function handleLanguageChange() {
  if (!state.currentUser && elements.authBtnText) {
    elements.authBtnText.setAttribute('data-i18n', 'header.connect_github');
    elements.authBtnText.textContent = t('header.connect_github');
  }
  if (elements.selectExistingRepo && elements.selectExistingRepo.options.length) {
    syncRepoSelection();
  }
  if (state.extensions.length) renderExtensionsGrid(state.extensions);
  if (state.analyzedSites.length) {
    elements.analysisCards.innerHTML = '';
    state.analyzedSites.forEach(renderAnalysisCard);
  }
  renderReleaseTable(state.lastRelease);
  refreshBackupExtensions();
  if (state.selectedBackupExt) loadBackups(state.selectedBackupExt);
  renderToolchainStatus();
  renderEngines();
  renderEngineTrials();
}

function initTabs() {
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
}

async function onDomReady() {
  appendTerminalLog('INFO', t('logs.init'));
  await checkAuthStatus();
  // Detección automática al abrir AEXT: si un módulo ya no está en su repo,
  // la ficha pasa a «Eliminadas» sin que el usuario tenga que buscarla.
  await checkRepoState();
  await loadExtensions();
  refreshBackupExtensions();

  try {
    const status = await invokeBackend('get_system_status');
    if (status && status.app_version) {
      state.appVersion = status.app_version;
      if (elements.appVersionBadge) elements.appVersionBadge.textContent = `v${status.app_version}`;
      if (elements.settingsCurrentVersion) elements.settingsCurrentVersion.textContent = `v${status.app_version}`;
    }
  } catch (_) {
    // El endpoint es opcional (fallback HTTP puede no exponerlo).
  }

  // La tabla de descargas se llena con los assets reales de la última release.
  await refreshReleaseInfo();

  // Aviso automático (una vez por versión instalada) para actualizar las
  // extensiones ya generadas a la siguiente versión de código.
  maybeShowUpdatePrompt();
}

// ==================== APP INITIALIZATION ====================
initExternalLinks();
window.addEventListener('aext:language', handleLanguageChange);
initTabs();
initAuth();
initAnalysis();
initGenerationFlow();
initSuccessModal();
initExtensionActions();
initStreamTester();
initTools();
initEngines();

const initialUiLang = initLanguage();
initSettings(initialUiLang);
initReleases();
initLogControls();

window.addEventListener('DOMContentLoaded', onDomReady);
