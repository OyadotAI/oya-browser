/** One keyboard vocabulary for native dispatch, command hints and the searchable guide. */
import { SHORTCUT_TAB_COUNT } from './constants.ts';

/** Modifier combinations supported by the browser, matched exactly. */
type Modifiers = 'primary' | 'primary-shift' | 'primary-alt' | 'control' | 'control-shift' | 'alt' | 'none';
/** A binding: command, description, section, modifiers, key and optional physical code. */
export type Shortcut = readonly [string, string, string, Modifiers, string, string?];
/** Bindings are ordered with the preferred discoverable chord first. */
export const SHORTCUTS: readonly Shortcut[] = [
  ['address', 'Focus address bar', 'Browsing', 'primary', 'L'],
  ['reload', 'Reload or stop loading', 'Browsing', 'primary', 'R'],
  ['back', 'Go back', 'Browsing', 'alt', 'ArrowLeft'],
  ['forward', 'Go forward', 'Browsing', 'alt', 'ArrowRight'],
  ['back', 'Go back', 'Browsing', 'primary', '[', 'BracketLeft'],
  ['forward', 'Go forward', 'Browsing', 'primary', ']', 'BracketRight'],
  ['zoom-in', 'Zoom in', 'Browsing', 'primary', '='],
  ['zoom-out', 'Zoom out', 'Browsing', 'primary', '-'],
  ['zoom-reset', 'Reset page zoom', 'Browsing', 'primary', '0'],
  ['inspect', 'Inspect this page', 'Workspace', 'primary-alt', 'I', 'KeyI'],
  ['network', 'Network activity', 'Workspace', 'primary-alt', 'N', 'KeyN'],
  ['source', 'Page source', 'Workspace', 'primary-alt', 'S', 'KeyS'],
  ['library', 'History and bookmarks', 'Library', 'primary', 'Y'],
  ['bookmark', 'Toggle bookmark', 'Library', 'primary', 'D'],
  ['new-tab', 'New tab', 'Tabs', 'primary', 'T'],
  ['close-tab', 'Close tab', 'Tabs', 'primary', 'W'],
  ['reopen-tab', 'Reopen closed tab', 'Tabs', 'primary-shift', 'T'],
  ['next-tab', 'Next tab', 'Tabs', 'control', 'Tab'],
  ['previous-tab', 'Previous tab', 'Tabs', 'control-shift', 'Tab'],
  ['move-tab-left', 'Move tab left', 'Tabs', 'primary-shift', 'PageUp'],
  ['move-tab-right', 'Move tab right', 'Tabs', 'primary-shift', 'PageDown'],
  ['commands', 'Search commands and settings', 'Workspace', 'primary', 'K'],
  ['tools', 'Toggle workspace', 'Workspace', 'primary-shift', 'D'],
  ['ask', 'Focus Ask Oya', 'Workspace', 'primary-alt', 'A', 'KeyA'],
  ['record', 'Start or stop recording', 'Workspace', 'primary-alt', 'R', 'KeyR'],
  ['playbooks', 'Open playbooks', 'Workspace', 'primary-alt', 'P', 'KeyP'],
  ['routines', 'Open routines', 'Workspace', 'primary-alt', 'U', 'KeyU'],
  ['models', 'Model settings', 'Workspace', 'primary-alt', 'M', 'KeyM'],
  ['account', 'Account, profiles and imports', 'Workspace', 'primary', ','],
  ['shortcuts', 'Keyboard shortcuts', 'Help', 'primary', '/', 'Slash'],
  ['shortcuts', 'Keyboard shortcuts', 'Help', 'none', 'F1'],
  ['address', 'Focus address bar', 'Browsing', 'none', 'F6'],
  ['reload', 'Reload or stop loading', 'Browsing', 'primary-shift', 'R'],
  ['next-tab', 'Next tab', 'Tabs', 'primary-shift', ']', 'BracketRight'],
  ['previous-tab', 'Previous tab', 'Tabs', 'primary-shift', '[', 'BracketLeft'],
  ['next-tab', 'Next tab', 'Tabs', 'primary-alt', 'ArrowRight', 'ArrowRight'],
  ['previous-tab', 'Previous tab', 'Tabs', 'primary-alt', 'ArrowLeft', 'ArrowLeft'],
  ...Array.from({ length: SHORTCUT_TAB_COUNT }, (_, i): Shortcut => [
    `tab-${i + 1}`,
    i + 1 === SHORTCUT_TAB_COUNT ? 'Select last tab' : `Select tab ${i + 1}`,
    'Tabs',
    'primary',
    String(i + 1),
    `Digit${i + 1}`,
  ]),
];
/** The portable portion of Electron keyboard input. */
export interface ShortcutInput {
  /** Only keyDown is dispatched. */
  type: string;
  /** Character from the keyboard layout. */
  key: string;
  /** Physical key, when supplied. */
  code: string;
  /** Repeats must not perform destructive actions. */
  isAutoRepeat: boolean;
  /** Shift modifier. */
  shift: boolean;
  /** Control modifier. */
  control: boolean;
  /** Option or Alt modifier. */
  alt: boolean;
  /** Command or Meta modifier. */
  meta: boolean;
}
/** Accepts both Node and navigator platform spellings. */
export const isMac = (platform: string): boolean => /mac|darwin/i.test(platform);
/** The exact modifier mask, avoiding accidental commands with extra keys held. */
function modifierMatch(input: ShortcutInput, modifiers: Modifiers, platform: string): boolean {
  const primary = modifiers.startsWith('primary');
  const control = modifiers.startsWith('control') || (primary && !isMac(platform));
  return (
    !!input.meta === (primary && isMac(platform)) &&
    !!input.control === control &&
    !!input.shift === modifiers.endsWith('shift') &&
    !!input.alt === modifiers.endsWith('alt')
  );
}
/** Resolves only registered keys, including layout-independent physical aliases. */
export function resolveShortcut(input: ShortcutInput, platform: string): string | undefined {
  if (input.type !== 'keyDown' || input.isAutoRepeat) return undefined;
  return SHORTCUTS.find(
    ([, , , mods, key, code]) =>
      modifierMatch(input, mods, platform) &&
      (code
        ? input.code === code || (!input.code && input.key.toLowerCase() === key.toLowerCase())
        : input.key.toLowerCase() === key.toLowerCase()),
  )?.[0];
}
/** Human-readable keys use the current operating system's vocabulary. */
export function shortcutLabel(shortcut: Shortcut, platform: string): string {
  const mac = isMac(platform);
  const mods = shortcut[3]
    .replace('primary', mac ? '⌘' : 'Ctrl')
    .replace('control', 'Ctrl')
    .replace('shift', mac ? '⇧' : 'Shift')
    .replace('alt', mac ? '⌥' : 'Alt')
    .replace('none', '');
  return [...mods.split('-').filter(Boolean), shortcut[4]].join(' ');
}
/** Preferred binding for a command, or no hint when the command has no chord. */
export function commandShortcut(command: string, platform: string): string {
  const shortcut = SHORTCUTS.find(([id]) => id === command);
  return shortcut ? shortcutLabel(shortcut, platform) : '';
}
