/**
 * The workspace panel's tabs (Record, Ask, Inspect) and the Inspect sub-tabs:
 * which is selected, their ARIA wiring, and keyboard navigation.
 */
/* global oyaBrowser, Dom, ShellState, DevPanel */
/* exported ToolNav */

/** The panel's tab navigation. */
const ToolNav = {
  /** Panes grouped under the Inspect tab. */
  INSPECT: ['actions', 'network', 'source'],

  /** Selects the tab for `pane` and wires tabs and panels to each other. */
  sync(pane) {
    const inspect = ToolNav.INSPECT.includes(pane);
    Dom.byId('inspect-nav').hidden = !inspect;
    document.querySelectorAll('.dev-tab').forEach((button) => ToolNav.syncTab(button, pane, inspect));
    document.querySelectorAll('.dev-pane').forEach(ToolNav.syncPanel);
    document.querySelectorAll('[data-inspect]').forEach((button) => {
      button.classList.toggle('active', button.dataset.inspect === pane);
      button.setAttribute('aria-pressed', String(button.dataset.inspect === pane));
    });
  },

  /** One tab: selected state, focusability, and the panel it controls. */
  syncTab(button, pane, inspect) {
    const active = button.dataset.pane === pane || (button.id === 'inspect-tab' && inspect);
    button.classList.toggle('active', active);
    button.setAttribute('aria-selected', String(active));
    button.tabIndex = active ? 0 : -1;
    button.id ||= 'tool-tab-' + button.dataset.pane;
    const controls = button.id === 'inspect-tab' && inspect ? pane : button.dataset.pane;
    button.setAttribute('aria-controls', 'pane-' + controls);
  },

  /** One panel: a tabpanel labelled by its tab. */
  syncPanel(panel) {
    panel.setAttribute('role', 'tabpanel');
    const name = panel.id.slice('pane-'.length);
    panel.setAttribute('aria-labelledby', ToolNav.INSPECT.includes(name) ? 'inspect-tab' : 'tool-tab-' + name);
  },

  /** The index a roving-focus key moves to among `count` items. */
  nextIndex(key, current, count) {
    if (key === 'Home') return 0;
    if (key === 'End') return count - 1;
    return (current + (key === 'ArrowRight' ? 1 : count - 1)) % count;
  },

  /** Arrow keys, Home and End move between the tabs. */
  keydown(event) {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    const items = [...document.querySelectorAll('.dev-tab')];
    const current = items.indexOf(document.activeElement);
    if (current < 0) return;
    event.preventDefault();
    const index = ToolNav.nextIndex(event.key, current, items.length);
    items[index].click();
    items[index].focus();
  },
};

ToolNav.sync(ShellState.activeDevPane);
document
  .querySelectorAll('[data-inspect]')
  .forEach((button) => button.addEventListener('click', () => DevPanel.show(button.dataset.inspect)));
document.querySelector('.dev-panel-header').addEventListener('keydown', ToolNav.keydown);
Dom.byId('tools-close').addEventListener('click', () => {
  oyaBrowser.toggleDevPanel();
  Dom.byId('btn-dev').focus();
});
oyaBrowser.onDevPanelState((open) => Dom.byId('btn-dev').setAttribute('aria-expanded', String(open)));
