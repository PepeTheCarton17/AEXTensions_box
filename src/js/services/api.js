// Llamadas directas a los endpoints de diagnóstico del servidor local.
const AI_API_BASE = "http://127.0.0.1:45731/api/ai";

export async function fetchAiDiagnostics() {
  const res = await fetch(`${AI_API_BASE}/diagnostics`);
  return await res.json();
}

export async function postAiPatch(patch) {
  const res = await fetch(`${AI_API_BASE}/patch`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(patch)
  });
  return { ok: res.ok, data: await res.json() };
}
