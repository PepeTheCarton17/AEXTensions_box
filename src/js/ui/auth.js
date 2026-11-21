// Sesión de GitHub (CLI o PAT) y cabecera de la aplicación.
import { state, elements, appendTerminalLog } from '../state.js';
import { invokeBackend } from '../services/tauri.js';
import { t } from '../i18n/index.js';

// ==================== GITHUB AUTHENTICATION ====================
async function checkAuthStatus() {
  try {
    const user = await invokeBackend('get_gh_auth_status');
    updateAuthUI(user);
  } catch (err) {
    console.warn('Error verificando sesión GitHub:', err);
    updateAuthUI(null);
  }
}

function updateAuthUI(user) {
  state.currentUser = user;
  if (user) {
    // Al iniciar sesión el botón muestra el usuario, no el texto traducible.
    elements.authBtnText.removeAttribute('data-i18n');
    elements.authBtnText.textContent = `@${user.username}`;
    if (user.avatar_url) {
      elements.headerAvatar.src = user.avatar_url;
      elements.headerAvatar.classList.remove('hidden');
    }
    // Modal
    elements.authLoggedInView.classList.remove('hidden');
    elements.authLoggedOutView.classList.add('hidden');
    elements.modalUserAvatar.src = user.avatar_url || 'https://github.com/ghost.png';
    elements.modalUserName.textContent = user.display_name || user.username;
    elements.modalUserLogin.textContent = `@${user.username}`;
    elements.modalAuthTypeBadge.textContent = user.auth_type === 'cli' ? 'Sesión CLI' : 'Token PAT';
  } else {
    elements.authBtnText.setAttribute('data-i18n', 'header.connect_github');
    elements.authBtnText.textContent = t('header.connect_github');
    elements.headerAvatar.classList.add('hidden');
    elements.authLoggedInView.classList.add('hidden');
    elements.authLoggedOutView.classList.remove('hidden');
  }
}

function initAuth() {
  elements.btnOpenAuthModal.addEventListener('click', () => {
    elements.modalGithubAuth.classList.remove('hidden');
    elements.modalGithubAuth.classList.add('flex');
  });

  elements.btnCloseAuthModal.addEventListener('click', () => {
    elements.modalGithubAuth.classList.add('hidden');
    elements.modalGithubAuth.classList.remove('flex');
  });

  if (elements.btnLoginCli) {
    elements.btnLoginCli.addEventListener('click', async () => {
      try {
        appendTerminalLog('INFO', 'Buscando credenciales en GitHub CLI local...');
        const info = await invokeBackend('login_with_cli');
        updateAuthUI(info);
        appendTerminalLog('SUCCESS', `Sesión iniciada con CLI: @${info.username}`);
        elements.modalGithubAuth.classList.add('hidden');
        elements.modalGithubAuth.classList.remove('flex');
      } catch (e) {
        appendTerminalLog('WARN', `CLI no detectado: ${e}. Ingresa un Personal Access Token.`);
        alert(t('alerts.cli_missing', { error: e }));
      }
    });
  }

  elements.btnSubmitPat.addEventListener('click', async () => {
    const token = elements.inputPatToken.value.trim();
    if (!token) {
      alert(t('alerts.token_required'));
      return;
    }
    try {
      appendTerminalLog('INFO', 'Verificando token con la API de GitHub...');
      const info = await invokeBackend('login_with_token', { token });
      updateAuthUI(info);
      appendTerminalLog('SUCCESS', `Sesión iniciada con éxito para @${info.username}`);
      elements.inputPatToken.value = '';
      elements.modalGithubAuth.classList.add('hidden');
      elements.modalGithubAuth.classList.remove('flex');
    } catch (e) {
      appendTerminalLog('ERROR', `Error de autenticación: ${e}`);
      alert(t('alerts.token_invalid', { error: e }));
    }
  });

  elements.btnSwitchAccount.addEventListener('click', () => {
    elements.authLoggedInView.classList.add('hidden');
    elements.authLoggedOutView.classList.remove('hidden');
  });

  elements.btnLogout.addEventListener('click', async () => {
    try {
      await invokeBackend('logout_gh');
      updateAuthUI(null);
      appendTerminalLog('INFO', 'Sesión de GitHub cerrada');
    } catch (e) {
      console.error(e);
    }
  });
}

export {
  checkAuthStatus,
  updateAuthUI,
  initAuth,
};

