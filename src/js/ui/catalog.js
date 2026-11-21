// Catálogo de extensiones: carga, filtros de repo y renderizado de tarjetas.
import { state, elements, escapeHtml, isRepoMissing, fallbackIcon, iconFor, appendTerminalLog } from '../state.js';
import { invokeBackend, saveLocalZip } from '../services/tauri.js';
import { t } from '../i18n/index.js';
import { normalizeSearchText, filteredExtensions } from '../services/search.js';
import { openExtensionFolder, refreshBackupExtensions } from './shared.js';

function refreshRepoFilters() {
  const repos = [...new Set(state.extensions.filter((e) => !isRepoMissing(e)).map((e) => e.github_repo_url).filter(Boolean))]
    .map((url) => url.replace(/\/$/, ''))
    .sort();
  for (const select of [elements.extRepoFilter, elements.updatePromptRepo]) {
    if (!select) continue;
    const previous = select.value;
    select.innerHTML = '';
    const all = document.createElement('option');
    all.value = '';
    all.textContent = t('ext.all_repos');
    all.setAttribute('data-i18n', 'ext.all_repos');
    select.appendChild(all);
    for (const repo of repos) {
      const option = document.createElement('option');
      option.value = repo;
      option.textContent = repo;
      select.appendChild(option);
    }
    select.value = repos.includes(previous) ? previous : '';
  }
}

async function loadExtensions() {
  try {
    const list = await invokeBackend('get_created_extensions') || [];
    state.extensions = list;
    refreshRepoFilters();
    elements.badgeExtCount.textContent = filteredExtensions(list.filter((e) => !isRepoMissing(e))).length;
    renderExtensionsGrid(list);
  } catch (e) {
    console.error('Error cargando extensiones:', e);
  }
}

function renderExtensionsGrid(list = state.extensions) {
  renderDeletedExtensionsGrid(list);
  const visible = filteredExtensions(list).filter((ext) => !isRepoMissing(ext));
  if (visible.length === 0) {
    const searching = Boolean(normalizeSearchText(elements.extSearch?.value || ''));
    elements.extensionsCatalogGrid.innerHTML = `
      <div class="col-span-full py-12 text-center text-slate-500 text-xs">
        ${escapeHtml(t(searching ? 'ext.no_results' : 'ext.empty'))}
      </div>
    `;
    return;
  }

  elements.extensionsCatalogGrid.innerHTML = '';
  for (const ext of visible) {
    const card = document.createElement('div');
    card.className = 'p-4 rounded-2xl bg-surface-900 border border-surface-800 space-y-4 hover:border-slate-700 transition-all shadow-lg';

    const icon = iconFor(ext);
    const iconFallback = fallbackIcon(ext.name);
    const downloadUrl = ext.apk_download_url || '';
    const isRealApk = /\.apk(\?|$)/i.test(downloadUrl);
    const hasGithub = Boolean(ext.github_repo_url);

    let primaryAction;
    if (downloadUrl && isRealApk) {
      primaryAction = `
        <a href="${escapeHtml(downloadUrl)}" class="flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-brand-600 hover:bg-brand-500 text-white flex items-center justify-center gap-1.5 transition-all text-center">
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" /></svg>
          <span>${escapeHtml(t('succ.apk'))}</span>
        </a>`;
    } else if (downloadUrl) {
      primaryAction = `
        <button data-download-url="${escapeHtml(downloadUrl)}" data-download-name="${escapeHtml(ext.pkg_id)}.zip" class="btn-download-zip flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-brand-600 hover:bg-brand-500 text-white flex items-center justify-center gap-1.5 transition-all text-center">
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" /></svg>
          <span>${escapeHtml(t('ext.download_zip'))}</span>
        </button>`;
    } else if (hasGithub) {
      primaryAction = `
        <a href="${escapeHtml(ext.github_repo_url.replace(/\/$/, ''))}/actions" target="_blank" rel="noreferrer" class="flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-brand-600 hover:bg-brand-500 text-white flex items-center justify-center gap-1.5 transition-all text-center" title="La compilación del APK corre en GitHub Actions">
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" /></svg>
          <span>${escapeHtml(t('ext.actions'))}</span>
        </a>`;
    } else {
      primaryAction = `
        <span class="flex-1 py-2 px-3 rounded-xl text-xs font-semibold bg-surface-800 text-slate-500 flex items-center justify-center gap-1.5 text-center">
          ${escapeHtml(t('ext.no_download'))}
        </span>`;
    }

    let buildBadge = '';
    if (ext.build_status === 'building') {
      buildBadge = `<span class="mt-1 inline-flex items-center gap-1 text-[10px] font-semibold text-amber-300"><span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse"></span>${escapeHtml(t('ext.building'))}</span>`;
    } else if (ext.build_status === 'pending') {
      buildBadge = `<span class="mt-1 inline-flex items-center gap-1 text-[10px] font-semibold text-amber-300" title="${escapeHtml(ext.build_error || '')}"><span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span>${escapeHtml(t('ext.pending'))}</span>`;
    } else if (ext.build_status === 'success') {
      buildBadge = `<span class="mt-1 inline-flex items-center gap-1 text-[10px] font-semibold text-emerald-300"><span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>${escapeHtml(t('ext.build_ok'))}</span>`;
    } else if (ext.build_status === 'error') {
      buildBadge = `<span class="mt-1 inline-flex items-center gap-1 text-[10px] font-semibold text-rose-300" title="${escapeHtml(ext.build_error || '')}"><span class="w-1.5 h-1.5 rounded-full bg-rose-400"></span>${escapeHtml(t('ext.build_error'))}</span>`;
    }
    // El badge apunta al run REAL de Actions, no solo al repo, para poder ver
    // qué falló. Antes AEXT mostraba éxito aunque el run estuviera en rojo.
    if (buildBadge && ext.actions_run_url) {
      buildBadge = `<a href="${escapeHtml(ext.actions_run_url)}" target="_blank" rel="noreferrer" class="inline-block" title="Abrir run de GitHub Actions">${buildBadge}</a>`;
    }

    card.innerHTML = `
      <div class="flex items-start justify-between">
        <div class="flex items-center gap-3">
          <img class="w-11 h-11 rounded-xl bg-surface-950 object-cover ring-1 ring-white/10" src="${escapeHtml(icon)}" onerror="this.onerror=null;this.src='${iconFallback}'" alt="${escapeHtml(ext.name)}">
          <div>
            <h3 class="text-sm font-bold text-slate-100 flex items-center gap-1.5">
              <span>${escapeHtml(ext.name)}</span>
              <span class="text-[10px] font-mono px-1.5 py-0.2 rounded bg-surface-800 text-slate-300 font-normal">v${escapeHtml(ext.version)}</span>
            </h3>
            <p class="text-[11px] font-mono text-slate-400">${escapeHtml(ext.pkg_id)}</p>
            ${buildBadge}
          </div>
        </div>
        <span class="px-2 py-0.5 rounded text-[10px] font-semibold bg-brand-500/10 text-brand-300 border border-brand-500/20 uppercase">${escapeHtml(ext.ext_type)}</span>
      </div>

      <div class="grid grid-cols-2 gap-2 text-[11px] pt-1 border-t border-surface-800/60">
        <div>
          <span class="text-slate-500 block text-[10px]">${escapeHtml(t('ext.cms'))}</span>
          <span class="text-slate-300 font-medium capitalize">${escapeHtml(ext.cms)}</span>
        </div>
        <div>
          <span class="text-slate-500 block text-[10px]">${escapeHtml(t('ext.audio'))}</span>
          <span class="text-slate-300 font-medium">${ext.has_sub_dub ? escapeHtml(t('ext.audio_config')) : escapeHtml(t('ext.audio_standard'))}</span>
        </div>
      </div>

      <!-- Action Buttons -->
      <div class="pt-2 border-t border-surface-800 flex flex-wrap items-center gap-2">
        ${primaryAction}

        ${isRealApk && hasGithub ? `
          <a href="${escapeHtml(downloadUrl)}" class="p-2 rounded-xl text-slate-500 hover:text-emerald-400 hover:bg-emerald-500/10 transition-all" title="${escapeHtml(t('ext.download_apk'))}" aria-label="${escapeHtml(t('ext.download_apk'))}">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" /></svg>
          </a>
        ` : ''}

        <button data-update-pkg="${escapeHtml(ext.pkg_id)}" class="btn-update-ext py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700 flex items-center gap-1.5 transition-all" title="${escapeHtml(t('ext.update'))}">
          <svg class="w-3.5 h-3.5 text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" /></svg>
          <span>${escapeHtml(t('ext.update'))}</span>
        </button>

        ${hasGithub && ext.actions_commit_sha ? `
          <button data-verify-actions-pkg="${escapeHtml(ext.pkg_id)}" class="btn-verify-actions py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700 flex items-center gap-1.5 transition-all" title="${escapeHtml(t('ext.verify_actions'))}">
            <svg class="w-3.5 h-3.5 text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" /></svg>
            <span>${escapeHtml(t('ext.verify_actions'))}</span>
          </button>
        ` : ''}

        <button data-open-folder-pkg="${escapeHtml(ext.pkg_id)}" class="btn-open-folder py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700 flex items-center gap-1.5 transition-all" title="${escapeHtml(t('ext.open_folder'))}">
          <svg class="w-3.5 h-3.5 text-brand-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/></svg>
          <span>${escapeHtml(t('ext.open_folder'))}</span>
        </button>

        ${ext.nekoyomi_repo_url ? `
          <button data-repo-url="${escapeHtml(ext.nekoyomi_repo_url)}" class="btn-copy-repo py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700 flex items-center gap-1.5 transition-all" title="Copiar enlace de repositorio para Nekoyomi">
            <svg class="w-3.5 h-3.5 text-brand-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" /></svg>
            <span>${escapeHtml(t('ext.copy_repo'))}</span>
          </button>
        ` : ''}

        ${ext.github_repo_url ? `
          <a href="${escapeHtml(ext.github_repo_url)}" target="_blank" class="py-2 px-3 rounded-xl text-xs font-medium bg-surface-800 hover:bg-surface-700 text-slate-300 border border-surface-700 flex items-center gap-1 transition-all" title="${escapeHtml(t('ext.github'))}">
            <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24"><path fill-rule="evenodd" clip-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" /></svg>
          </a>
        ` : ''}

        ${ext.github_repo_url ? `
          <button data-delete-github-pkg="${escapeHtml(ext.pkg_id)}" class="btn-delete-ext-github p-2 rounded-xl text-slate-500 hover:text-amber-400 hover:bg-amber-500/10 transition-all" title="${escapeHtml(t('ext.delete_github'))}">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 15a4 4 0 004 4h9a5 5 0 00-.1-10 6 6 0 00-11.6 1.5A3.5 3.5 0 003 15zm7-4l4 4m0-4l-4 4" /></svg>
          </button>
        ` : ''}

        <!-- Delete button -->
        <button data-delete-pkg="${escapeHtml(ext.pkg_id)}" class="btn-delete-ext p-2 rounded-xl text-slate-500 hover:text-red-400 hover:bg-red-500/10 transition-all" title="${escapeHtml(t('ext.delete'))}">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
        </button>
      </div>
    `;

    elements.extensionsCatalogGrid.appendChild(card);
  }

  // Hook copy buttons
  document.querySelectorAll('.btn-copy-repo').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const url = e.currentTarget.getAttribute('data-repo-url');
      if (!url) return;
      try {
        await navigator.clipboard.writeText(url);
        alert(t('alerts.copy_repo'));
      } catch (_) {
        alert(t('alerts.copy_repo_fallback', { url }));
      }
    });
  });

  // Hook open-folder buttons
  document.querySelectorAll('.btn-open-folder').forEach(btn => {
    btn.addEventListener('click', (e) => {
      const pkgId = e.currentTarget.getAttribute('data-open-folder-pkg');
      if (pkgId) openExtensionFolder(pkgId);
    });
  });

  // Hook local ZIP download buttons (Blob: nunca navega fuera de la app)
  document.querySelectorAll('.btn-download-zip').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const button = e.currentTarget;
      const url = button.getAttribute('data-download-url');
      const filename = button.getAttribute('data-download-name') || 'extension.zip';
      if (!url) return;
      const label = button.querySelector('span');
      const original = label ? label.textContent : '';
      button.disabled = true;
      button.classList.add('opacity-60', 'cursor-wait');
      if (label) label.textContent = t('ext.downloading');
      try {
        const saved = await saveLocalZip(url, filename);
        appendTerminalLog('SUCCESS', t('logs.zip_saved', { file: filename }));
        if (saved && saved.path) {
          appendTerminalLog('INFO', t('logs.zip_path', { path: saved.path }));
        }
        if (label) label.textContent = `✓ ${t('ext.download_zip')}`;
      } catch (error) {
        appendTerminalLog('ERROR', t('logs.zip_error', { file: filename, error: error.message || error }));
        alert(t('alerts.zip_failed', { error: error.message || error }));
        if (label) label.textContent = original || t('ext.download_zip');
      } finally {
        button.disabled = false;
        button.classList.remove('opacity-60', 'cursor-wait');
      }
    });
  });

  // Hook update buttons
  document.querySelectorAll('.btn-update-ext').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const button = e.currentTarget;
      const pkgId = button.getAttribute('data-update-pkg');
      if (!pkgId) return;
      const label = button.querySelector('span');
      const original = label ? label.textContent : '';
      button.disabled = true;
      button.classList.add('opacity-60', 'cursor-wait');
      if (label) label.textContent = t('ext.updating');
      appendTerminalLog('INFO', `↻ ${pkgId}…`);
      try {
        // El idioma elegido en ajustes se aplica también al regenerar.
        const lang = localStorage.getItem('aext_lang') || 'es';
        const updated = await invokeBackend('update_extension', { pkgId, lang });
        appendTerminalLog('SUCCESS', t('logs.updated', {
          name: updated ? updated.name : pkgId,
          version: updated ? updated.version : '?',
        }));
        await loadExtensions();
        refreshBackupExtensions();
      } catch (err) {
        appendTerminalLog('ERROR', t('logs.update_error', { pkg: pkgId, error: err }));
        alert(t('logs.update_error', { pkg: pkgId, error: err }));
        button.disabled = false;
        button.classList.remove('opacity-60', 'cursor-wait');
        if (label) label.textContent = original || t('ext.update');
      }
    });
  });

  // Hook verify-Actions buttons: consulta el run real y actualiza el badge.
  document.querySelectorAll('.btn-verify-actions').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const button = e.currentTarget;
      const pkgId = button.getAttribute('data-verify-actions-pkg');
      if (!pkgId) return;
      const label = button.querySelector('span');
      const original = label ? label.textContent : '';
      button.disabled = true;
      button.classList.add('opacity-60', 'cursor-wait');
      if (label) label.textContent = t('ext.verifying');
      appendTerminalLog('INFO', `↻ ${t('ext.verify_actions')}: ${pkgId}…`);
      try {
        await invokeBackend('refresh_github_actions', { pkgId });
        await loadExtensions();
      } catch (err) {
        appendTerminalLog('ERROR', String(err));
        alert(String(err));
        button.disabled = false;
        button.classList.remove('opacity-60', 'cursor-wait');
        if (label) label.textContent = original || t('ext.verify_actions');
      }
    });
  });

  // Hook delete buttons
  document.querySelectorAll('.btn-delete-ext').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const pkgId = e.currentTarget.getAttribute('data-delete-pkg');
      if (!confirm(t('alerts.confirm_delete', { pkg: pkgId }))) return;
      try {
        await invokeBackend('delete_extension', { pkgId });
        appendTerminalLog('SUCCESS', t('logs.moved_to_deleted', { pkg: pkgId }));
        await loadExtensions();
        refreshBackupExtensions();
      } catch (err) {
        const detail = String(err);
        appendTerminalLog('ERROR', detail);
        alert(detail);
      }
    });
  });

  // Hook delete-from-GitHub buttons: confirma la ruta exacta del módulo para
  // que el usuario sepa que NO se borra el repositorio ni otros módulos.
  document.querySelectorAll('.btn-delete-ext-github').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      const pkgId = e.currentTarget.getAttribute('data-delete-github-pkg');
      if (!pkgId) return;
      const parts = pkgId.split('.');
      const modulePath = `src/${parts[parts.length - 2]}/${parts[parts.length - 1]}`;
      if (!confirm(t('alerts.confirm_delete_github', { pkg: pkgId, module: modulePath }))) return;
      try {
        await invokeBackend('delete_extension_github', { pkgId });
        appendTerminalLog('SUCCESS', t('logs.moved_to_deleted', { pkg: pkgId }));
        await loadExtensions();
        refreshBackupExtensions();
      } catch (err) {
        const detail = String(err);
        appendTerminalLog('ERROR', detail);
        alert(detail);
      }
    });
  });

}

function renderDeletedExtensionsGrid(list = state.extensions) {
  const section = elements.deletedExtensionsSection;
  const grid = elements.deletedExtensionsGrid;
  if (!section || !grid) return;

  const visible = filteredExtensions(list).filter(isRepoMissing);
  if (visible.length === 0) {
    section.classList.add('hidden');
    grid.innerHTML = '';
    return;
  }

  section.classList.remove('hidden');
  if (elements.deletedExtCount) elements.deletedExtCount.textContent = String(visible.length);
  grid.innerHTML = '';

  for (const ext of visible) {
    const icon = escapeHtml(iconFor(ext));
    const iconFallback = escapeHtml(fallbackIcon(ext.name));
    const card = document.createElement('div');
    card.className = 'p-4 rounded-2xl bg-surface-900/60 border border-rose-500/20 opacity-80 space-y-3';
    card.innerHTML = `
      <div class="flex items-start gap-3">
        <img class="w-10 h-10 rounded-xl bg-surface-950 object-cover ring-1 ring-white/10" src="${icon}" onerror="this.onerror=null;this.src='${iconFallback}'" alt="${escapeHtml(ext.name)}">
        <div class="min-w-0 flex-1">
          <h3 class="text-sm font-bold text-slate-300 truncate">${escapeHtml(ext.name)} <span class="ml-1 text-[10px] font-semibold px-1.5 py-0.5 rounded bg-rose-500/10 text-rose-300 border border-rose-500/20">${escapeHtml(t('ext.deleted_badge'))}</span></h3>
          <p class="text-[11px] font-mono text-slate-500 truncate">${escapeHtml(ext.pkg_id)}</p>
          <p class="mt-1 text-[11px] text-slate-500">${escapeHtml(t('ext.deleted_hint'))}</p>
        </div>
      </div>
      <div class="flex flex-wrap gap-2 border-t border-surface-800 pt-3">
        <button data-restore-repo-pkg="${escapeHtml(ext.pkg_id)}" class="btn-restore-ext-repo px-3 py-2 rounded-xl text-xs font-semibold bg-surface-800 hover:bg-surface-700 text-slate-200 border border-surface-700 transition-all">
          ${escapeHtml(t('ext.restore'))}
        </button>
        <button data-delete-pkg="${escapeHtml(ext.pkg_id)}" class="btn-delete-ext px-3 py-2 rounded-xl text-xs font-medium bg-surface-800 hover:bg-red-500/10 text-slate-400 hover:text-red-400 border border-surface-700 transition-all" title="${escapeHtml(t('ext.delete_permanent'))}">
          ${escapeHtml(t('ext.delete_permanent'))}
        </button>
      </div>
    `;
    grid.appendChild(card);
  }

  grid.querySelectorAll('.btn-restore-ext-repo').forEach((btn) => {
    btn.addEventListener('click', async (e) => {
      const pkgId = e.currentTarget.getAttribute('data-restore-repo-pkg');
      if (!pkgId) return;
      if (!confirm(t('alerts.confirm_restore_repo', { pkg: pkgId }))) return;
      const button = e.currentTarget;
      button.disabled = true;
      try {
        await invokeBackend('restore_extension_repo_state', { pkgId });
        appendTerminalLog('SUCCESS', t('logs.restored_repo', { pkg: pkgId }));
        await loadExtensions();
      } catch (err) {
        const detail = String(err);
        appendTerminalLog('ERROR', detail);
        alert(detail);
        button.disabled = false;
      }
    });
  });

  grid.querySelectorAll('.btn-delete-ext').forEach((btn) => {
    btn.addEventListener('click', async (e) => {
      const pkgId = e.currentTarget.getAttribute('data-delete-pkg');
      if (!confirm(t('alerts.confirm_delete_permanent', { pkg: pkgId }))) return;
      try {
        await invokeBackend('delete_extension_permanently', { pkgId });
        appendTerminalLog('SUCCESS', t('logs.deleted_permanently', { pkg: pkgId }));
        await loadExtensions();
        refreshBackupExtensions();
      } catch (err) {
        const detail = String(err);
        appendTerminalLog('ERROR', detail);
        alert(detail);
      }
    });
  });
}

export {
  refreshRepoFilters,
  loadExtensions,
  renderExtensionsGrid,
  renderDeletedExtensionsGrid,
};

