// Preferencias de idioma/repo y controles de la terminal.
import { state, elements, appendTerminalLog } from '../state.js';
import { t, setLanguage, getLanguage } from '../i18n/index.js';
import { loadGithubRepos } from './generator.js';

function initSettings(initialUiLang) {
  // ==================== IDIOMA DE LA INTERFAZ ====================
  if (elements.settingsAppUiLang) {
    elements.settingsAppUiLang.value = initialUiLang;
    elements.settingsAppUiLang.addEventListener('change', () => {
      setLanguage(elements.settingsAppUiLang.value);
    });
  }
  // ==================== SETTINGS SYNC ====================
  // Sync language selector between Generator tab and Settings tab
  if (elements.settingsLang && elements.selectExtLang) {
    // Load saved language from localStorage
    const savedLang = localStorage.getItem('aext_lang') || 'es';
    elements.settingsLang.value = savedLang;
    elements.selectExtLang.value = savedLang;

    elements.settingsLang.addEventListener('change', () => {
      elements.selectExtLang.value = elements.settingsLang.value;
    });
    elements.selectExtLang.addEventListener('change', () => {
      elements.settingsLang.value = elements.selectExtLang.value;
    });
  }

  // Load saved default repo
  if (elements.settingsDefaultRepo) {
    const savedRepo = localStorage.getItem('aext_default_repo') || '';
    elements.settingsDefaultRepo.value = savedRepo;
  }

  // Save settings button
  if (elements.btnSaveSettings) {
    elements.btnSaveSettings.addEventListener('click', () => {
      const lang = elements.settingsLang?.value || 'es';
      const repo = elements.settingsDefaultRepo?.value?.trim() || '';
      const uiLang = elements.settingsAppUiLang?.value || getLanguage();

      localStorage.setItem('aext_lang', lang);
      localStorage.setItem('aext_default_repo', repo);
      if (uiLang !== getLanguage()) setLanguage(uiLang);

      // Sync generator lang selector
      if (elements.selectExtLang) elements.selectExtLang.value = lang;

      if (state.currentUser) loadGithubRepos();

      appendTerminalLog('SUCCESS', t('logs.settings_saved', { lang, repo: repo || '(auto)' }));
      alert(t('alerts.settings_saved'));
    });
  }
}

function initLogControls() {
  elements.btnClearLogs.addEventListener('click', () => {
    elements.terminalLogs.innerHTML = '';
  });
}

export {
  initSettings,
  initLogControls,
};

