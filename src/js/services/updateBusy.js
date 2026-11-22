// Bloqueo global de actualizaciones: una sola operación (individual, por lote
// o masiva) a la vez. También evita que los botones del modal se solapen.
import { state, elements } from '../state.js';

const GLOBAL_BUTTONS = [
  'btnSyncExtensions',
  'btnUpdateAll',
  'btnUpdateRepo',
  'btnUpdatePromptAll',
  'btnUpdatePromptRepo',
  'btnUpdatePromptLater',
];

function setBulkBusy(busy) {
  state.updatesBusy = Boolean(busy);
  const buttons = [
    ...GLOBAL_BUTTONS.map((key) => elements[key]),
    ...document.querySelectorAll('.btn-update-ext, .btn-verify-actions'),
  ];
  for (const button of buttons) {
    if (!button) continue;
    button.disabled = state.updatesBusy;
    button.classList.toggle('opacity-60', state.updatesBusy);
    button.classList.toggle('cursor-wait', state.updatesBusy);
  }
}

export { setBulkBusy };
