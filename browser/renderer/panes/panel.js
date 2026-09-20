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
    document.querySelectorAll('.dev-tab').forEach((t) => t.classList.toggle('active', t.dataset.pane === pane));
    document.querySelectorAll('.dev-pane').forEach((p) => p.classList.toggle('active', p.id === 'pane-' + pane));
    ShellState.activeDevPane = pane;
    ToolNav.sync(pane);
    oyaBrowser.saveUiPreferences({ pane });
  },

  /** The panel opened or closed. */
  opened(open) {
    ShellState.devOpen = open;
    Dom.byId('dev-panel').inert = !open;
    Dom.byId('btn-dev').classList.toggle('active', open);
  },

  /** The Oya Agent button: opens the panel on Ask (on Record while recording), or closes it. */
  async toggle() {
    const opening = !ShellState.devOpen;
    await oyaBrowser.toggleDevPanel();
    if (!opening) return;
    DevPanel.show(ShellState.recording ? 'record' : 'chat');
    if (!ShellState.recording) Dom.byId('chat-input').focus();
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

document.querySelectorAll('.dev-tab').forEach((tab) => {
  tab.addEventListener('click', () => {
    if (tab.dataset.pane) DevPanel.show(tab.dataset.pane);
  });
});
Dom.byId('btn-dev').addEventListener('click', DevPanel.toggle);
oyaBrowser.onDevPanelState(DevPanel.opened);
Dom.byId('dev-clear').addEventListener('click', DevPanel.clear);
