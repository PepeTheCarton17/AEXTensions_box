// Análisis determinista de URLs y tarjetas de sitios detectados.
import { state, elements, escapeHtml, fallbackIcon, appendTerminalLog } from '../state.js';
import { invokeBackend } from '../services/tauri.js';
import { t } from '../i18n/index.js';

// ==================== URL ANALYZER ====================
function getEnteredUrls() {
  const text = elements.inputUrls.value;
  return text.split('\n')
    .map(u => u.trim())
    .filter(u => u.startsWith('http://') || u.startsWith('https://'));
}

function renderAnalysisCard(site) {
  const card = document.createElement('div');
  card.className = 'p-4 rounded-xl bg-surface-950 border border-surface-800 space-y-3';

  const hostsBadges = site.detected_hosts.length > 0
    ? site.detected_hosts.map(h => `<span class="px-1.5 py-0.5 rounded text-[10px] bg-indigo-500/10 text-indigo-300 border border-indigo-500/20">${escapeHtml(h)}</span>`).join(' ')
    : '<span class="text-slate-500 text-[10px]">No detectados</span>';

  const protectionsBadges = site.protections.length > 0
    ? site.protections.map(p => `<span class="px-1.5 py-0.5 rounded text-[10px] bg-amber-500/10 text-amber-300 border border-amber-500/20">${escapeHtml(p)}</span>`).join(' ')
    : '<span class="text-slate-500 text-[10px]">Ninguna</span>';

  const caps = site.capabilities || {};
  const methodNames = {
    'session-bootstrap': 'Sesión web',
    'graphql-persisted': 'GraphQL interno',
    'hls-master': 'HLS',
    'dash-mpd': 'DASH',
    'iframe-player': 'Reproductor incrustado',
    'drm-widevine': 'DRM detectado',
    'ads-hls': 'Anuncios HLS',
  };
  const methods = Array.isArray(caps.playback_methods) && caps.playback_methods.length > 0
    ? caps.playback_methods
    : (Array.isArray(caps.player_cases) ? caps.player_cases.map(c => methodNames[c] || c) : []);
  const verifiedAudio = caps.master_verified
    ? [caps.sub_verified && 'SUB', caps.dub_verified && 'DUB'].filter(Boolean)
    : [];
  const capBadges = (values, colors) => (Array.isArray(values) && values.length > 0)
    ? values.map(v => {
        const text = String(v);
        const short = text.length > 46 ? text.slice(0, 46) + '…' : text;
        return `<span class="px-1.5 py-0.5 rounded text-[10px] ${colors}" title="${escapeHtml(text)}">${escapeHtml(short)}</span>`;
      }).join(' ')
    : '<span class="text-slate-500 text-[10px]">No detectado</span>';

  const capabilitiesHtml = `
    <div class="space-y-1 pt-1 border-t border-surface-800">
      <span class="text-slate-500 block text-[10px]">${escapeHtml(t('analysis.capabilities'))}</span>
      <div><span class="text-[10px] text-slate-400">Formatos:</span> ${capBadges(caps.stream_formats, 'bg-emerald-500/10 text-emerald-300 border border-emerald-500/20')}</div>
      <div><span class="text-[10px] text-slate-400">Calidades:</span> ${capBadges(caps.qualities, 'bg-sky-500/10 text-sky-300 border border-sky-500/20')}</div>
      <div><span class="text-[10px] text-slate-400">Audio:</span> ${capBadges(caps.audio_languages, 'bg-violet-500/10 text-violet-300 border border-violet-500/20')}</div>
      <div><span class="text-[10px] text-slate-400">Subtítulos:</span> ${capBadges(caps.subtitle_languages, 'bg-violet-500/10 text-violet-300 border border-violet-500/20')}</div>
      <div><span class="text-[10px] text-slate-400">${escapeHtml(t('analysis.endpoints'))}:</span> ${capBadges(caps.api_endpoints, 'bg-cyan-500/10 text-cyan-300 border border-cyan-500/20')}</div>
      <div><span class="text-[10px] text-slate-500">${escapeHtml(t('analysis.endpoints_note'))}</span></div>
      <div><span class="text-[10px] text-slate-400">App:</span> ${caps.app_version ? escapeHtml(caps.app_version) : '<span class="text-slate-500 text-[10px]">No detectada</span>'}</div>
      <div><span class="text-[10px] text-slate-400">${escapeHtml(t('analysis.region'))}:</span> ${
        caps.region || caps.locale
          ? `${caps.region ? escapeHtml(caps.region) : ''}${caps.locale ? ` · ${escapeHtml(caps.locale)}` : ''}`
          : '<span class="text-slate-500 text-[10px]">No detectada</span>'
      }</div>
      <div><span class="text-[10px] text-slate-500">${escapeHtml(t('analysis.region_note'))}</span></div>
      <div><span class="text-[10px] text-slate-400">${escapeHtml(t('analysis.methods'))}:</span> ${capBadges(methods, 'bg-amber-500/10 text-amber-300 border border-amber-500/20')}</div>
    </div>`;

  const warnings = Array.isArray(site.warnings) ? site.warnings : [];
  const samples = Array.isArray(site.sample_items) ? site.sample_items : [];
  const searchValue = site.search_path
    ? (site.search_param ? `${site.search_path} · ${site.search_param}` : site.search_path)
    : null;
  const routes = [
    { label: t('analysis.listing'), value: site.popular_path },
    { label: t('analysis.latest'), value: site.latest_path },
    { label: t('analysis.search'), value: searchValue },
  ];
  const routesHtml = routes.map(route => `
      <div>
        <span class="text-slate-500 block text-[10px]">${escapeHtml(route.label)}</span>
        <span class="font-mono text-[10px] ${route.value ? 'text-emerald-300' : 'text-slate-500'}">${route.value ? escapeHtml(route.value) : escapeHtml(t('analysis.not_detected'))}</span>
      </div>`).join('');

  const confidenceLabel = site.catalog_detected
    ? t('analysis.confidence_ok')
    : t('analysis.confidence_low');
  const confidenceClass = site.catalog_detected
    ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/20'
    : 'bg-amber-500/10 text-amber-300 border-amber-500/20';

  const samplesHtml = samples.length > 0
    ? `<div class="space-y-1">
        <span class="text-slate-500 block text-[10px]">${escapeHtml(t('analysis.samples'))}</span>
        <div class="flex flex-wrap gap-1">${samples.map(s => `<span class="px-1.5 py-0.5 rounded text-[10px] bg-surface-800 text-slate-300 border border-surface-700">${escapeHtml(s)}</span>`).join('')}</div>
      </div>`
    : '';

  const warningsHtml = warnings.length > 0
    ? `<div class="space-y-1 pt-2 border-t border-amber-500/20">
        <span class="text-amber-300 block text-[10px] font-semibold">${escapeHtml(t('analysis.warnings'))}</span>
        <ul class="list-disc list-inside space-y-0.5 text-[10px] text-amber-200/80">
          ${warnings.map(w => `<li>${escapeHtml(w)}</li>`).join('')}
        </ul>
      </div>`
    : '';

  const catalogSourceLabels = {
    html: 'analysis.catalog_html',
    jsonld: 'analysis.catalog_jsonld',
    none: 'analysis.catalog_none',
  };
  const catalogSourceLabel = catalogSourceLabels[site.catalog_source] || 'analysis.catalog_none';
  const catalogSourceClass = site.catalog_source === 'jsonld'
    ? 'bg-sky-500/10 text-sky-300 border border-sky-500/20'
    : site.catalog_source === 'html'
      ? 'bg-emerald-500/10 text-emerald-300 border border-emerald-500/20'
      : 'bg-slate-500/10 text-slate-400 border border-slate-500/20';
  const ldTitles = Array.isArray(site.catalog_records) ? site.catalog_records : [];
  const searchAction = site.search_action || null;
  const filterParams = Array.isArray(site.filter_params) ? site.filter_params : [];
  const filterPairs = [...new Set(filterParams
    .filter(f => f && f.param && f.value)
    .map(f => `${f.param}=${f.value}`))];
  const catalogHtml = `
    <div class="space-y-1 pt-2 border-t border-surface-800">
      <span class="text-slate-500 block text-[10px]">${escapeHtml(t('analysis.catalog_source'))}</span>
      <div><span class="px-1.5 py-0.5 rounded text-[10px] ${catalogSourceClass}">${escapeHtml(t(catalogSourceLabel))}</span></div>
      ${
        site.catalog_source === 'none' || !site.catalog_detected
          ? `<div><span class="px-1.5 py-0.5 rounded text-[10px] bg-amber-500/10 text-amber-300 border border-amber-500/20">${escapeHtml(t('analysis.engines_pending'))}</span></div>
             <div><span class="text-[10px] text-amber-200/80">${escapeHtml(t('analysis.engines_note'))}</span></div>`
          : ''
      }
      <div><span class="text-[10px] text-slate-400">${escapeHtml(t('analysis.page_language'))}:</span> ${
        site.detected_language
          ? escapeHtml(site.detected_language)
          : `<span class="text-slate-500">${escapeHtml(t('analysis.not_detected'))}</span>`
      }</div>
      <div><span class="text-[10px] text-slate-400">${escapeHtml(t('analysis.search_action'))}:</span> ${
        searchAction && searchAction.url_template
          ? `<span class="font-mono text-[10px] text-emerald-300" title="${escapeHtml(searchAction.url_template)}">${escapeHtml(searchAction.url_template)}</span>`
          : `<span class="text-slate-500 text-[10px]">${escapeHtml(t('analysis.not_detected'))}</span>`
      }</div>
      <div><span class="text-[10px] text-slate-400">${escapeHtml(t('analysis.filters'))}:</span> ${capBadges(filterPairs, 'bg-amber-500/10 text-amber-300 border border-amber-500/20')}</div>
      ${
        samples.length === 0 && ldTitles.length > 0
          ? `<div><span class="text-slate-500 block text-[10px]">${escapeHtml(t('analysis.samples'))}</span><div class="flex flex-wrap gap-1">${ldTitles.slice(0, 10).map(r => `<span class="px-1.5 py-0.5 rounded text-[10px] bg-surface-800 text-slate-300 border border-surface-700" title="${escapeHtml(r.url || '')}">${escapeHtml(r.alternate_title || r.title)}</span>`).join('')}</div></div>`
          : ''
      }
    </div>`;

  const trials = Array.isArray(site.engine_trials) ? site.engine_trials : [];
  const engineTrialsHtml = trials.length > 0
    ? `<div class="space-y-1 pt-2 border-t border-surface-800">
        <span class="text-slate-500 block text-[10px]">${escapeHtml(t('analysis.engine_trials'))}</span>
        <div class="flex flex-wrap gap-1">${trials.map((trial) => {
          const ok = Boolean(trial.success);
          const name = escapeHtml(trial.engine_name || trial.engine_id);
          const detail = escapeHtml(`${trial.probe_id}: ${trial.detail}`);
          return `<span class="px-1.5 py-0.5 rounded text-[10px] ${ok ? 'bg-emerald-500/10 text-emerald-300 border border-emerald-500/20' : 'bg-slate-700/40 text-slate-400'}" title="${detail}">${ok ? '' : '✗ '}${name} · ${trial.content_count}</span>`;
        }).join('')}</div>
        <div class="text-[10px] text-slate-500">${escapeHtml(t('analysis.engine_trials_note'))}</div>
      </div>`
    : '';

  const siteIcon = site.favicon_url && site.favicon_url.trim()
    ? site.favicon_url
    : fallbackIcon(site.name);
  const siteIconFallback = fallbackIcon(site.name);

  card.innerHTML = `
    <div class="flex items-start justify-between gap-3">
      <div class="flex items-center gap-3">
        <img class="w-8 h-8 rounded-lg bg-surface-800 object-cover" src="${escapeHtml(siteIcon)}" onerror="this.onerror=null;this.src='${siteIconFallback}'">
        <div>
          <h4 class="text-xs font-bold text-slate-100">${escapeHtml(site.name)}</h4>
          <span class="text-[10px] font-mono text-slate-400" title="${escapeHtml(site.entry_url || site.url)}">${escapeHtml(site.url)}</span>
        </div>
      </div>
      <div class="flex flex-col items-end gap-1">
        <span class="px-2 py-0.5 rounded text-[10px] font-semibold bg-brand-500/10 text-brand-300 border border-brand-500/20 uppercase">${escapeHtml(site.ext_type)}</span>
        <span class="px-2 py-0.5 rounded text-[10px] font-semibold border ${confidenceClass}">${escapeHtml(confidenceLabel)}</span>
      </div>
    </div>

    <div class="grid grid-cols-2 gap-2 text-[11px] pt-1">
      <div>
        <span class="text-slate-500 block text-[10px]">Motor / CMS</span>
        <span class="font-medium text-slate-300 capitalize">${escapeHtml(site.cms)}</span>
      </div>
      <div>
        <span class="text-slate-500 block text-[10px]">Pistas SUB/DUB verificadas</span>
        <span class="font-medium ${verifiedAudio.length ? 'text-emerald-400' : 'text-slate-400'}">${verifiedAudio.length ? escapeHtml(verifiedAudio.join(' + ')) : 'Sin verificar (no se ofrecerá)'}</span>
      </div>
    </div>

    <div class="space-y-1 pt-2 border-t border-surface-800">
      <span class="text-slate-500 block text-[10px]">${escapeHtml(t('analysis.detected_paths'))}</span>
      <div class="grid grid-cols-3 gap-2">${routesHtml}</div>
      <div class="pt-1">
        <span class="text-slate-500 block text-[10px]">${escapeHtml(t('analysis.episodes'))}</span>
        <span class="font-mono text-[10px] ${site.episode_selector ? 'text-emerald-300' : 'text-amber-300'}">${escapeHtml(site.episode_selector && site.episode_selector.trim() ? site.episode_selector : t('analysis.episodes_generic'))}</span>
      </div>
    </div>

    ${samplesHtml}

    ${catalogHtml}

    ${engineTrialsHtml}

    <div class="space-y-1 pt-1 border-t border-surface-800">
      <span class="text-slate-500 block text-[10px]">Servidores detectados:</span>
      <div class="flex flex-wrap gap-1">${hostsBadges}</div>
    </div>

    <div class="space-y-1">
      <span class="text-slate-500 block text-[10px]">Protecciones WAF / Cloudflare:</span>
      <div class="flex flex-wrap gap-1">${protectionsBadges}</div>
    </div>

    ${capabilitiesHtml}

    ${warningsHtml}
  `;

  elements.analysisCards.appendChild(card);
}

function initAnalysis() {
  elements.btnAnalyzeUrls.addEventListener('click', async () => {
    const urls = getEnteredUrls();
    if (urls.length === 0) {
      alert(t('alerts.urls_required'));
      return;
    }

    appendTerminalLog('INFO', `Iniciando análisis determinista para ${urls.length} sitio(s)...`);
    elements.analysisCards.innerHTML = '';
    elements.analysisPreviewContainer.classList.remove('hidden');
    state.analyzedSites = [];

    for (const url of urls) {
      try {
        appendTerminalLog('INFO', `Analizando DOM y CMS en: ${url}`);
        const result = await invokeBackend('analyze_url', { url });
        state.analyzedSites.push(result);
        renderAnalysisCard(result);
        appendTerminalLog('SUCCESS', `Sitio identificado: ${result.name} (CMS: ${result.cms}, Servidores: ${result.detected_hosts.join(', ') || 'N/A'})`);
        for (const warning of (result.warnings || [])) {
          appendTerminalLog('WARN', `${result.name}: ${warning}`);
        }
      } catch (e) {
        appendTerminalLog('ERROR', `Fallo al analizar ${url}: ${e}`);
      }
    }
  });
}

export {
  getEnteredUrls,
  renderAnalysisCard,
  initAnalysis,
};

