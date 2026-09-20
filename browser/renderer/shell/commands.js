/**
 * The command palette: searchable shell commands with their shortcuts, and
 * the shortcuts the main process forwards from the menu.
 */
/* global oyaBrowser, Dom, ShellState, DevPanel, ShellDialog, Updates, StudioActions */
/* exported CommandPalette */

/** The palette. */
/** The palette's commands: label, keys after the platform modifier ('' for none), action. */
const COMMANDS = [
  ['Focus address bar', 'L', () => CommandPalette.focusAddress()],
  ['New tab', 'T', () => oyaBrowser.newTab()],
  ['Open Oya Agent', '⇧ D', () => DevPanel.toggle()],
  ['Record a workflow', '⇧ R', () => StudioActions.recordButton()],
  ['Ask Oya', '', () => (CommandPalette.openPane('chat'), Dom.byId('chat-input').focus())],
  ['Inspect this page', '', () => CommandPalette.openPane('actions')],
  ['Connection and profile', '', () => ShellDialog.open(true)],
  ['Check for updates', '', async () => Updates.render(await oyaBrowser.checkForUpdates())],
];

const CommandPalette = {
  /** ⌘ on a Mac, Ctrl elsewhere. */
  modifier: navigator.platform.includes('Mac') ? '⌘' : 'Ctrl',

  /** Focuses and selects the address bar. */
  focusAddress() {
    Dom.byId('url-bar').focus();
    Dom.byId('url-bar').select();
  },

  /** Opens the workspace panel (if closed) on `pane`. */
  openPane(pane) {
    if (!ShellState.devOpen) oyaBrowser.toggleDevPanel();
    DevPanel.show(pane);
  },

  /** Every command: label, shortcut (with this platform's modifier), action. */
  commands() {
    const m = CommandPalette.modifier;
    return COMMANDS.map(([label, keys, run]) => [label, keys && m + ' ' + keys, run]);
  },

  /** A button for one command; it closes the dialog, then runs. */
  button([label, shortcut, run]) {
    const button = Dom.node('button', null, 'command');
    button.append(Dom.node('span', label), Dom.node('kbd', shortcut));
    button.addEventListener('click', async () => {
      ShellDialog.close();
      await run();
    });
    return button;
  },

  /** Lists the commands matching the search. */
  render() {
    const list = Dom.byId('command-list');
    const query = Dom.byId('command-search').value.toLowerCase();
    list.replaceChildren();
    for (const command of CommandPalette.commands().filter(([label]) => label.toLowerCase().includes(query))) {
      list.append(CommandPalette.button(command));
    }
    if (!list.children.length) list.append(Dom.node('p', 'No matching commands.'));
  },

  /** A fresh, focused search over every command. */
  reset() {
    Dom.byId('command-search').value = '';
    CommandPalette.render();
    Dom.byId('command-search').focus();
  },

  /** Down moves into the list; Enter runs the first match. */
  searchKey(event) {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      document.querySelector('.command')?.focus();
    }
    if (event.key === 'Enter') document.querySelector('.command')?.click();
  },

  /** Up and Down move between commands, wrapping around. */
  listKey(event) {
    if (!['ArrowDown', 'ArrowUp'].includes(event.key)) return;
    event.preventDefault();
    const buttons = [...document.querySelectorAll('.command')];
    const index = buttons.indexOf(document.activeElement);
    buttons[(index + (event.key === 'ArrowDown' ? 1 : buttons.length - 1)) % buttons.length]?.focus();
  },

  /** Menu shortcuts forwarded by the main process. */
  SHORTCUTS: {
    address: () => CommandPalette.focusAddress(),
    commands: () => ShellDialog.open(),
    record: () => StudioActions.recordButton(),
    tools: () => DevPanel.toggle(),
  },

  /** Runs a forwarded shortcut; unknown ones are ignored. */
  shortcut(command) {
    if (Object.hasOwn(CommandPalette.SHORTCUTS, command)) CommandPalette.SHORTCUTS[command]();
  },
};

Dom.byId('command-search').addEventListener('input', CommandPalette.render);
Dom.byId('command-search').addEventListener('keydown', CommandPalette.searchKey);
Dom.byId('command-list').addEventListener('keydown', CommandPalette.listKey);
oyaBrowser.onShellCommand(CommandPalette.shortcut);
