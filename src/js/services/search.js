// Búsqueda tolerante a errores sobre el catálogo de extensiones.
import { state, elements } from '../state.js';

function normalizeSearchText(value) {
  return String(value || '')
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
}

// Distancia de edición con transposición de caracteres adyacentes: tolera
// errores de una letra y también que el usuario intercambie dos ("frmae").
function editDistance(a, b) {
  if (a === b) return 0;
  if (!a.length) return b.length;
  if (!b.length) return a.length;
  let prev2 = null;
  let prev = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    const curr = new Array(b.length + 1);
    curr[0] = i;
    for (let j = 1; j <= b.length; j++) {
      let best = Math.min(
        curr[j - 1] + 1,
        prev[j] + 1,
        prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1)
      );
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) {
        best = Math.min(best, prev2[j - 2] + 1);
      }
      curr[j] = best;
    }
    prev2 = prev;
    prev = curr;
  }
  return prev[b.length];
}

function lcsLength(a, b) {
  if (!a.length || !b.length) return 0;
  let prev = new Array(b.length + 1).fill(0);
  for (let i = 1; i <= a.length; i++) {
    const curr = new Array(b.length + 1).fill(0);
    for (let j = 1; j <= b.length; j++) {
      curr[j] = a[i - 1] === b[j - 1]
        ? prev[j - 1] + 1
        : Math.max(prev[j], curr[j - 1]);
    }
    prev = curr;
  }
  return prev[b.length];
}

function tokenMatchScore(query, token) {
  if (!query || !token) return -1;
  if (token === query) return 1;
  if (token.startsWith(query)) return 0.96;
  const at = token.indexOf(query);
  if (at >= 0) {
    return 0.9 - Math.min(0.08, at * 0.01) - Math.min(0.05, (token.length - query.length) * 0.002);
  }
  if (query.length < 3 || token.length < 3) return -1;
  const lev = 1 - editDistance(query, token) / Math.max(query.length, token.length);
  // Cuántas letras escritas aparecen, en orden, en el título candidato. Así
  // "puta" encuentra "pluto3" sin que "guayaba" encuentre nada parecido.
  const coverage = lcsLength(query, token) / query.length;
  // Exigir una coincidencia real de letras (o una distancia muy corta) evita
  // que un par de caracteres sueltos arrastren extensiones sin parentesco.
  if (coverage < 0.7 && lev < 0.8) return -1;
  const gap = Math.abs(query.length - token.length);
  const penalty = gap > Math.max(3, query.length) ? 0.15 : 0;
  const score = Math.max(lev * 0.95, coverage * 0.9) - penalty;
  return score >= 0.62 ? score : -1;
}

function searchScore(query, text) {
  if (!query) return 0;
  if (!text) return -1;
  const qTokens = query.split(/\s+/).filter(Boolean);
  const cTokens = text.split(/\s+/).filter(Boolean);
  if (!cTokens.length) return -1;
  if (qTokens.length === 1) {
    return Math.max(...cTokens.map((token) => tokenMatchScore(qTokens[0], token)));
  }
  let total = 0;
  for (const q of qTokens) {
    let best = -1;
    for (const c of cTokens) best = Math.max(best, tokenMatchScore(q, c));
    if (best < 0) return -1;
    total += best;
  }
  return total / qTokens.length;
}

function extensionSearchText(ext) {
  // Solo datos de identidad de la extensión. El repo de GitHub, el tipo y el
  // idioma son comunes a todas las variantes: si entraran en el índice,
  // buscar "puta" o "Pluto3" también sacaría Frame por el nombre del repo.
  return normalizeSearchText([
    ext.name,
    ext.pkg_id,
    ext.class_name,
    ext.base_url,
  ].join(' '));
}

function filteredExtensions(list = state.extensions) {
  const query = normalizeSearchText(elements.extSearch?.value || '');
  const repo = (elements.extRepoFilter?.value || '').replace(/\/$/, '');
  const scored = [];
  for (const ext of list) {
    if (repo && (ext.github_repo_url || '').replace(/\/$/, '') !== repo) continue;
    if (!query) {
      scored.push({ ext, score: 0 });
      continue;
    }
    const score = searchScore(query, extensionSearchText(ext));
    if (score >= 0) scored.push({ ext, score });
  }
  if (query) {
    scored.sort((a, b) => b.score - a.score || String(a.ext.name).localeCompare(String(b.ext.name)));
  }
  return scored.map((entry) => entry.ext);
}

export {
  normalizeSearchText,
  filteredExtensions,
};

