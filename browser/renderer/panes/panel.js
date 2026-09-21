/**
 * The workspace panel: opening and closing it, switching panes, and clearing
 * the pane in view.
 */
/* global oyaBrowser, Dom, ShellState, ToolNav, Chat, NetLog, SourcePane, DevActions */
/* exported DevPanel */

/** The workspace panel. */
const DevPanel = {
  /** Shows `pane`, keeps the tabs in step, and remembers the choice. */
  show(pane) {
    document.querySelectorAll('.dev-pane').forEach((p) => p.classList.toggle('active', p.id === 'pane-' + pane));
    ShellState.activeDevPane = pane;
    if (ToolNav.INSPECT.includes(pane)) ShellState.lastInspect = pane;
    ToolNav.sync(pane);
    oyaBrowser.saveUiPreferences({ pane });
  },

  /** The panel opened or closed. */
  opened(open) {
    ShellState.devOpen = open;
    Dom.byId('dev-panel').inert = !open;
    Dom.byId('btn-dev').classList.toggle('active', open);
  },

  /** The Oya Agent button: opens the panel where it was left (on Record while recording), or closes it. */
  async toggle() {
    const opening = !ShellState.devOpen;
    await oyaBrowser.toggleDevPanel();
    if (!opening) return;
    DevPanel.show(ShellState.recording ? 'record' : ShellState.activeDevPane);
    if (ShellState.activeDevPane === 'chat') Dom.byId('chat-input').focus();
  },

  /** A tab was clicked: the Inspect tab reopens its last sub-pane. */
  tabClicked(tab) {
    if (tab.id === 'inspect-tab') return DevPanel.show(ShellState.lastInspect);
    if (tab.dataset.pane) DevPanel.show(tab.dataset.pane);
  },

  /** What Clear does in each pane. */
  CLEARS: {
    chat: () => Chat.clear(),
    network: () => NetLog.clear(),
    source: () => SourcePane.clear(),
    actions: () => DevActions.clear(),
  },

  /** Clears the pane in view; panes without a clear action are left alone. */
  clear() {
    const pane = ShellState.activeDevPane;
    if (Object.hasOwn(DevPanel.CLEARS, pane)) DevPanel.CLEARS[pane]();
  },
};

document.querySelectorAll('.dev-tab').forEach((tab) => tab.addEventListener('click', () => DevPanel.tabClicked(tab)));
Dom.byId('btn-dev').addEventListener('click', DevPanel.toggle);
oyaBrowser.onDevPanelState(DevPanel.opened);
Dom.byId('dev-clear').addEventListener('click', DevPanel.clear);
