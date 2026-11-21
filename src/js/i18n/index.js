// Traducciones de la interfaz (es / en). Las claves se usan desde index.html
// con data-i18n / data-i18n-ph / data-i18n-title y desde los módulos con t().
import { es } from './es.js';
import { en } from './en.js';

export const I18N = { es, en };

let activeLang = 'es';

export function setLanguage(lang) {
  activeLang = I18N[lang] ? lang : 'es';
  try {
    localStorage.setItem('aext_ui_lang', activeLang);
  } catch (_) {
    // localStorage puede no estar disponible en algunos webviews.
  }
  applyLanguage(activeLang);
  return activeLang;
}

export function getLanguage() {
  return activeLang;
}

export function initLanguage() {
  let saved = 'es';
  try {
    saved = localStorage.getItem('aext_ui_lang') || 'es';
  } catch (_) {
    saved = 'es';
  }
  return setLanguage(saved);
}

export function t(key, vars = {}) {
  const dict = I18N[activeLang] || I18N.es;
  let text = dict[key] ?? I18N.es[key] ?? key;
  for (const [name, value] of Object.entries(vars)) {
    text = text.replaceAll(`{${name}}`, String(value));
  }
  return text;
}

export function applyLanguage(lang) {
  const dict = I18N[lang] || I18N.es;
  document.documentElement.lang = lang;

  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const value = dict[el.dataset.i18n];
    if (typeof value === 'string') el.textContent = value;
  });

  document.querySelectorAll('[data-i18n-ph]').forEach((el) => {
    const value = dict[el.dataset.i18nPh];
    if (typeof value === 'string') el.placeholder = value;
  });

  document.querySelectorAll('[data-i18n-title]').forEach((el) => {
    const value = dict[el.dataset.i18nTitle];
    if (typeof value === 'string') el.title = value;
  });

  // Las opciones de los selectores también se traducen.
  document.querySelectorAll('option[data-i18n]').forEach((el) => {
    const value = dict[el.dataset.i18n];
    if (typeof value === 'string') el.textContent = value;
  });

  window.dispatchEvent(new CustomEvent('aext:language', { detail: lang }));
}
