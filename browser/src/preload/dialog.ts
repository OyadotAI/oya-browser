/** Private sandboxed bridge on Oya's static dialog sheet, never on website content. */
import { ipcRenderer } from 'electron';
import { NATIVE_DIALOG, type DialogPresentation } from '../shared/native-dialog.ts';
/** Resolve only known controls from the packaged static sheet. */
const element = (id: string): HTMLElement => document.getElementById(id)!;
/** Website messages and defaults are text/value only, never interpreted as markup. */
function render(data: DialogPresentation): void {
  for (const key of ['origin', 'title', 'message', 'accept', 'cancel'] as const) element(key).textContent = data[key];
  const input = element('value') as HTMLInputElement;
  input.hidden = !data.prompt;
  input.value = data.value;
  element('content').hidden = false;
  focusInput(data.prompt, input);
  requestAnimationFrame(() => ipcRenderer.send(NATIVE_DIALOG.READY));
}
/** Enter submits prompt text; all other decisions initially focus the safe action. */
function focusInput(prompt: boolean, input: HTMLInputElement): void {
  if (prompt) {
    input.focus();
    input.select();
  } else element('cancel').focus();
}
/** Only an explicit submit accepts; Escape and the cancel button preserve the current page. */
function answer(accept: boolean): void {
  ipcRenderer.send(NATIVE_DIALOG.ANSWER, accept, (element('value') as HTMLInputElement).value);
}
/** Keyboard events remain inside the isolated preload and cannot be supplied by a website. */
function wire(): void {
  element('content').addEventListener('submit', submit);
  element('cancel').addEventListener('click', () => answer(false));
  document.addEventListener('keydown', escape);
  ipcRenderer
    .invoke(NATIVE_DIALOG.READ)
    .then(render)
    .catch(() => answer(false));
}
/** Prevent navigation from the form; only its native IPC reply is permitted. */
function submit(event: Event): void {
  event.preventDefault();
  answer(true);
}
/** Escape never silently accepts a confirmation or discards unsaved work. */
function escape(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    answer(false);
  }
}
document.addEventListener('DOMContentLoaded', wire, { once: true });
