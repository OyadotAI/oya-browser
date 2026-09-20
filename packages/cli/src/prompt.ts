/**
 * Terminal prompts. No dependency earns its place for this.
 *
 * On a TTY, menus are arrow-key selectable and redraw in place. Everywhere else
 * — a pipe, CI, a test — the same call falls back to a numbered list read line by
 * line, so scripted input behaves identically to typing.
 *
 * This file is the facade; the work is in prompt/: style (colour), frame
 * (banners, steps, spinners), lines (the shared input queue), ask (text
 * answers) and menu (choices).
 */
export { style, icon } from './prompt/style.ts';
export { banner, steps, step, note, success, warn, spinner } from './prompt/frame.ts';
export { closePrompts } from './prompt/lines.ts';
export { InputError, ask, askSecret, confirm, type AskOptions } from './prompt/ask.ts';
export { choose, type Option } from './prompt/menu.ts';
