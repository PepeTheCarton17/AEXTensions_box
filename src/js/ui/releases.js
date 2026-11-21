// Tabla de assets de la última release y comprobación de versiones.
import { state, elements, escapeHtml } from '../state.js';
import { invokeBackend, openExternal } from '../services/tauri.js';
import { t } from '../i18n/index.js';

function releaseAssetMeta(name) {
  const lower = String(name || '').toLowerCase();
  if (lower.endsWith('.apk')) {
    const arch = lower.includes('arm64') ? 'arm64-v8a' : 'universal';
    return { platform: 'Android (Mobile)', arch, tone: 'emerald', icon: 'android' };
  }
  if (lower.endsWith('.msi')) {
    return { platform: 'Windows (PC) MSI', arch: 'x86_64', tone: 'sky', icon: 'windows' };
  }
  if (lower.endsWith('setup.exe')) {
    return { platform: 'Windows (PC) Setup', arch: 'x86_64', tone: 'sky', icon: 'windows' };
  }
  if (lower.endsWith('.exe')) {
    return { platform: `Windows (PC) ${t('rel.portable')}`, arch: 'x86_64', tone: 'sky', icon: 'windows' };
  }
  if (lower.endsWith('.appimage')) {
    return { platform: 'Linux (PC) AppImage', arch: 'x86_64', tone: 'amber', icon: 'linux' };
  }
  if (lower.endsWith('.deb')) {
    return { platform: 'Linux (PC) Debian', arch: 'x86_64', tone: 'amber', icon: 'linux' };
  }
  return { platform: 'Archivo', arch: '—', tone: 'slate', icon: 'file' };
}

function releaseIcon(kind, tone) {
  if (kind === 'android') {
    return `<svg class="w-4 h-4 text-${tone}-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z"/></svg>`;
  }
  if (kind === 'windows') {
    return `<svg class="w-4 h-4 text-${tone}-400" fill="currentColor" viewBox="0 0 24 24"><path d="M0 3.449L9.75 2.1v9.451H0m10.949-9.602L24 0v11.4H10.949M0 12.6h9.75v9.451L0 20.699M10.949 12.6H24V24l-12.901-1.799"/></svg>`;
  }
  if (kind === 'linux') {
    return `<svg class="w-4 h-4 text-${tone}-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"/></svg>`;
  }
  return `<svg class="w-4 h-4 text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/></svg>`;
}

function renderReleaseTable(release) {
  if (!elements.releaseAssetsBody) return;
  state.lastRelease = release || state.lastRelease;
  const data = state.lastRelease;
  const assets = (data && Array.isArray(data.assets)) ? data.assets : [];

  if (!assets.length) {
    elements.releaseAssetsBody.innerHTML = `
      <tr>
        <td colspan="4" class="p-3 text-slate-500">${escapeHtml(t('rel.no_assets'))}</td>
      </tr>`;
    return;
  }

  const weight = (name) => {
    const lower = String(name).toLowerCase();
    if (lower.endsWith('setup.exe')) return 0;
    if (lower.endsWith('.msi')) return 1;
    if (lower.endsWith('.exe')) return 2;
    if (lower.endsWith('.appimage')) return 3;
    if (lower.endsWith('.deb')) return 4;
    if (lower.endsWith('.apk')) return 5;
    return 6;
  };

  const rows = assets
    .filter((asset) => /\.(exe|msi|apk|appimage|deb)$/i.test(asset.name || ''))
    .sort((a, b) => weight(a.name) - weight(b.name))
    .map((asset) => {
      const meta = releaseAssetMeta(asset.name);
      const size = formatBytes(asset.size);
      return `
        <tr class="hover:bg-surface-850/50">
          <td class="p-3 font-sans font-semibold text-slate-200 flex items-center gap-2">
            ${releaseIcon(meta.icon, meta.tone)}
            <span>${escapeHtml(meta.platform)}</span>
          </td>
          <td class="p-3 text-indigo-300 break-all">${escapeHtml(asset.name || '')}</td>
          <td class="p-3 text-slate-400">${escapeHtml(meta.arch)}</td>
          <td class="p-3 text-right whitespace-nowrap">
            <button data-open-url="${escapeHtml(asset.url || '')}" class="px-2.5 py-1 rounded bg-surface-800 hover:bg-surface-700 text-brand-300 border border-surface-700 transition-all">
              ${escapeHtml(t('rel.download_btn'))}${size ? ` · ${escapeHtml(size)}` : ''}
            </button>
          </td>
        </tr>`;
    });

  elements.releaseAssetsBody.innerHTML = rows.join('') || `
    <tr>
      <td colspan="4" class="p-3 text-slate-500">${escapeHtml(t('rel.no_assets'))}</td>
    </tr>`;
}

// ==================== VERSIONES Y DESCARGAS ====================
async function refreshReleaseInfo() {
  try {
    const data = await invokeBackend('check_aext_updates');
    if (data && data.current_version) {
      state.appVersion = data.current_version;
      if (elements.appVersionBadge) elements.appVersionBadge.textContent = `v${data.current_version}`;
      if (elements.settingsCurrentVersion) elements.settingsCurrentVersion.textContent = `v${data.current_version}`;
    }
    renderReleaseTable(data);
    return data;
  } catch (error) {
    renderReleaseTable(null);
    return null;
  }
}

function initReleases() {
  elements.btnCheckUpdates.addEventListener('click', async () => {
    const button = elements.btnCheckUpdates;
    const label = button.querySelector('span');
    if (label) label.textContent = t('rel.checking');
    button.disabled = true;
    try {
      const data = await refreshReleaseInfo();
      if (!data) {
        alert(t('alerts.update_error', { error: 'sin conexión con GitHub' }));
        return;
      }
      if (data.update_available) {
        const open = confirm(t('alerts.update_found', {
          latest: data.latest_version,
          current: data.current_version,
          notes: data.release_notes || '',
        }));
        if (open && data.release_url) {
          await openExternal(data.release_url);
        }
      } else {
        alert(t('alerts.up_to_date', {
          version: data.current_version,
          notes: data.release_notes || '',
        }));
      }
    } catch (e) {
      alert(t('alerts.update_error', { error: e }));
    } finally {
      button.disabled = false;
      if (label) label.textContent = t('rel.check');
    }
  });
}

export {
  renderReleaseTable,
  refreshReleaseInfo,
  initReleases,
};

