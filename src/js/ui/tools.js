// Prueba de streams, MCP, logs, parches y copias de seguridad.
import { state, elements, appendTerminalLog } from '../state.js';
import { invokeBackend } from '../services/tauri.js';
import { fetchAiDiagnostics, postAiPatch } from '../services/api.js';
import { t } from '../i18n/index.js';
import { refreshBackupExtensions, loadBackups, openExtensionFolder } from './shared.js';
import { loadExtensions } from './catalog.js';
import { loadToolchain } from './build.js';

function initStreamTester() {
  // ==================== STREAM EXTRACTOR TESTER & DIAGNOSTICS ====================
  elements.btnTestStream.addEventListener('click', async () => {
    const url = elements.testStreamInput.value.trim();
    if (!url) {
      alert(t('alerts.no_stream'));
      return;
    }

    elements.testStreamResult.classList.remove('hidden');
    elements.testStreamResult.innerHTML = `<span class="text-slate-400">Ejecutando prueba de conexión y bypass de cabeceras...</span>`;

    try {
      const res = await invokeBackend('test_stream_extractor', { url });
      if (res.success) {
        elements.testStreamResult.innerHTML = `
          <div class="text-emerald-400 font-bold">[OK] ÉXITO: El servidor autorizó la reproducción (${res.host})</div>
          <div class="text-slate-300">Calidad: ${res.quality || 'Automática'}</div>
          <div class="text-slate-500 text-[10px] mt-1">Cabeceras enviadas: Referer, Origin, Sec-Fetch-Mode: cors</div>
        `;
      } else {
        elements.testStreamResult.innerHTML = `
          <div class="text-rose-400 font-bold">[FALLO]: ${res.error || 'Acceso denegado'} (${res.host})</div>
          <div class="text-slate-400 text-[10px]">Causa probable: Cloudflare WAF requiere cookies o token de sesión temporal.</div>
        `;
      }
    } catch (e) {
      elements.testStreamResult.innerHTML = `<div class="text-rose-400">Error: ${e}</div>`;
    }
  });
}

function initTools() {
  // ==================== HERRAMIENTAS, MCP Y COPIAS DE SEGURIDAD ====================
  if (elements.btnCopyMcpConfig) {
    elements.btnCopyMcpConfig.addEventListener('click', () => {
      const config = {
        mcpServers: {
          aext: {
            url: "http://127.0.0.1:45731/mcp"
          }
        }
      };
      navigator.clipboard.writeText(JSON.stringify(config, null, 2));
      alert(t('alerts.mcp_copied'));
    });
  }

  if (elements.btnCopyLogsAi) {
    elements.btnCopyLogsAi.addEventListener('click', async () => {
      try {
        const logs = await invokeBackend('get_system_logs') || [];
        const text = logs.map(l => `[${l.timestamp}] [${l.level}] ${l.message}`).join('\n');
        navigator.clipboard.writeText(`LOGS AEXT:\n${text}`);
        alert(t('alerts.logs_copied'));
      } catch (e) {
        alert(t('alerts.logs_copy_error'));
      }
    });
  }

  if (elements.btnGenAiDiag) {
    elements.btnGenAiDiag.addEventListener('click', async () => {
      try {
        const diag = await fetchAiDiagnostics();
        navigator.clipboard.writeText(JSON.stringify(diag, null, 2));
        alert(t('alerts.diag_copied'));
      } catch (e) {
        alert(t('alerts.diag_error'));
      }
    });
  }

  if (elements.btnApplyAiPatch) {
    elements.btnApplyAiPatch.addEventListener('click', async () => {
      const val = elements.inputAiPatch.value.trim();
      if (!val) {
        alert('Ingresa el JSON del parche');
        return;
      }
      try {
        const patch = JSON.parse(val);
        const { ok, data } = await postAiPatch(patch);
        if (ok) {
          appendTerminalLog('SUCCESS', t('logs.patch_ok', { pkg: patch.pkg_id, version: data.version }));
          alert(t('alerts.patch_ok', {
            version: data.version,
            backup: data.backup || '—',
          }));
          const pkgId = patch.pkg_id;
          await loadExtensions();
          refreshBackupExtensions();
          if (pkgId) loadBackups(pkgId);
        } else {
          alert(t('alerts.patch_error', { error: data.error }));
        }
      } catch (e) {
        alert(t('alerts.invalid_json', { error: e.message }));
      }
    });
  }

  if (elements.selectBackupExt) {
    elements.selectBackupExt.addEventListener('change', () => {
      state.selectedBackupExt = elements.selectBackupExt.value;
      loadBackups(state.selectedBackupExt);
    });
  }

  if (elements.btnRefreshBackups) {
    elements.btnRefreshBackups.addEventListener('click', async () => {
      refreshBackupExtensions();
      await loadBackups(elements.selectBackupExt?.value || '');
    });
  }

  if (elements.btnOpenSelectedFolder) {
    elements.btnOpenSelectedFolder.addEventListener('click', () => {
      const pkgId = elements.selectBackupExt?.value || state.selectedBackupExt;
      if (!pkgId) {
        alert(t('alerts.missing_ext'));
        return;
      }
      openExtensionFolder(pkgId);
    });
  }

  if (elements.backupsList) {
    elements.backupsList.addEventListener('click', async (event) => {
      const button = event.target.closest('[data-restore-backup]');
      if (!button) return;
      const backup = button.getAttribute('data-restore-backup');
      const pkgId = elements.selectBackupExt?.value || state.selectedBackupExt;
      if (!backup || !pkgId) return;

      if (!confirm(`Restaurar ${backup} en ${pkgId}? Los archivos actuales se sobrescribirán.`)) {
        return;
      }

      button.disabled = true;
      try {
        const result = await invokeBackend('restore_extension_backup', { pkgId, backup });
        appendTerminalLog('SUCCESS', t('logs.restore_ok', {
          backup,
          count: result.restored_files,
        }));
        alert(t('alerts.restore_ok', { count: result.restored_files }));
        await loadExtensions();
        await loadBackups(pkgId);
      } catch (error) {
        appendTerminalLog('ERROR', String(error));
        alert(t('alerts.restore_error', { error }));
        button.disabled = false;
      }
    });
  }
}

export {
  initStreamTester,
  initTools,
};

