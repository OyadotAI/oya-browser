/**
 * The tab strip: one item per open page, kept in the main process's order,
 * sized like Chrome's (an even share of the strip, at most 240px, scrolling
 * once tabs reach their narrowest), plus the toolbar state that follows the
 * active tab. Tabs fold away when closed, and after a close with the mouse
 * their widths hold until the pointer leaves the strip, so the next close
 * button lands under it.
 */
/* global oyaBrowser, Dom, ShellIcons, ToolNav, TabItem, TabMath, TabDrag, TabCard, RendererConstants */
/* exported TabStrip */

/** The tab strip. */
const TabStrip = {
  /** The tabs as the main process last sent them. */
  tabs: [],
  /** A tab width held after a close with the mouse, until the pointer leaves the strip; else null. */
  frozen: null,
  /** Whether the strip has drawn once: tabs that open later animate in. */
  drawn: false,

  /** Brings the strip in line with `tabs`: folds closed ones away, adds and updates the rest, in order. */
  update(tabs) {
    TabStrip.tabs = tabs;
    const items = TabStrip.items();
    for (const item of items) if (!tabs.some((tab) => String(tab.id) === item.dataset.id)) TabStrip.fold(item);
    for (const tab of tabs) TabStrip.draw(items, tab);
    TabStrip.order(tabs);
    TabStrip.layout();
    TabStrip.drawn = true;
  },

  /** Draws one tab, making its item if it is new; the active one drives the toolbar. */
  draw(items, tab) {
    const item = items.find((node) => node.dataset.id === String(tab.id)) || TabStrip.create(tab);
    TabItem.render(item, tab);
    if (tab.active) TabStrip.activate(item, tab);
  },

  /** The open tabs' items, without those folding away. */
  items: () => [...Dom.byId('tab-list').querySelectorAll('.tab-item:not(.closing)')],

  /** The item for a new tab, added to the strip; one opened after the first draw grows in. */
  create(tab) {
    const item = TabItem.create(tab);
    if (TabStrip.drawn && !TabStrip.reducedMotion()) item.classList.add('opening');
    item.addEventListener('animationend', () => item.classList.remove('opening'));
    Dom.byId('tab-list').append(item);
    return item;
  },

  /** Whether the person asked for less motion. */
  reducedMotion: () => matchMedia('(prefers-reduced-motion: reduce)').matches,

  /** A closed tab folds to nothing, then goes; it is no longer a tab while it does. */
  fold(item) {
    if (TabStrip.reducedMotion()) return item.remove();
    item.classList.add('closing');
    item.classList.remove('active');
    item.inert = true;
    item.querySelector('.tab-title').removeAttribute('role');
    setTimeout(() => item.remove(), RendererConstants.TAB_CLOSE_MS);
  },

  /** Puts the items in the main process's order (not mid-drag), keeping focus on a moved tab. */
  order(tabs) {
    if (TabDrag.state) return;
    tabs.forEach((tab, i) => {
      const items = TabStrip.items();
      const item = items.find((node) => node.dataset.id === String(tab.id));
      if (items[i] !== item) TabStrip.place(item, items[i]);
    });
  },

  /** Moves `item` before `before`, keeping the keyboard focus it held (moving a node drops focus). */
  place(item, before) {
    const focused = item.contains(document.activeElement) && document.activeElement;
    Dom.byId('tab-list').insertBefore(item, before);
    if (focused) focused.focus();
  },

  /** Sizes the tabs: a held width, or an even share of the room, and the size they draw at. */
  layout() {
    const list = Dom.byId('tab-list');
    const width = TabStrip.frozen ?? TabMath.width(TabStrip.room(), TabStrip.tabs.length);
    list.style.setProperty('--tab-width', width + 'px');
    list.dataset.size = TabMath.size(width);
    list.classList.toggle('overflowing', TabStrip.tabs.length * width > TabStrip.room());
  },

  /** The width the tabs may share: from the strip's start to the menu button, less the new-tab button and the drag space. */
  room() {
    const list = Dom.byId('tab-list');
    const end = Dom.byId('btn-commands').getBoundingClientRect().left;
    const style = getComputedStyle(list);
    const padding = (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.paddingRight) || 0);
    const newTab = Dom.byId('btn-new-tab').offsetWidth;
    return end - list.getBoundingClientRect().left - padding - newTab - RendererConstants.TAB_DRAG_RESERVE;
  },

  /** Closes a tab the mouse closed, holding every width so the next close button lands under the pointer. */
  closeByMouse(id) {
    TabStrip.frozen ??= TabMath.width(TabStrip.room(), TabStrip.tabs.length);
    TabCard.hide();
    oyaBrowser.closeTab(id);
  },

  /** The pointer left the strip: held widths let go and the tabs fill the room again. */
  release() {
    if (TabStrip.frozen === null || TabDrag.state) return;
    TabStrip.frozen = null;
    TabStrip.layout();
  },

  /** The active tab drives the toolbar: progress, status, reload/stop and history buttons. */
  activate(item, tab) {
    Dom.byId('navigation-progress').hidden = !tab.loading;
    TabStrip.status(tab);
    TabStrip.reloadButton(tab.loading);
    Dom.byId('url-bar').setAttribute('aria-busy', String(tab.loading));
    Dom.byId('btn-back').disabled = !tab.canGoBack;
    Dom.byId('btn-forward').disabled = !tab.canGoForward;
    TabStrip.reveal(item);
  },

  /** The address bar's loading status. */
  status(tab) {
    const status = Dom.byId('navigation-status');
    status.textContent = tab.loadError ? 'Load failed' : tab.loading ? 'Loading…' : '';
    status.title = tab.loadError || '';
    status.setAttribute('aria-label', tab.loadError || status.textContent);
  },

  /** Reload becomes Stop while the page loads. */
  reloadButton(loading) {
    const reload = Dom.byId('btn-reload');
    reload.innerHTML = ShellIcons.icon(loading ? 'close' : 'reload');
    reload.setAttribute('aria-label', loading ? 'Stop loading' : 'Reload page');
    reload.title = loading ? 'Stop loading' : 'Reload page';
  },

  /** Scroll only the tab strip, never the entire renderer or open tools. */
  reveal(item) {
    const tabList = Dom.byId('tab-list');
    if (item.offsetLeft < tabList.scrollLeft) tabList.scrollLeft = item.offsetLeft;
    else if (item.offsetLeft + item.offsetWidth > tabList.scrollLeft + tabList.clientWidth) {
      tabList.scrollLeft = item.offsetLeft + item.offsetWidth - tabList.clientWidth;
    }
  },

  /** Arrow keys, Home and End move between tabs; Delete closes the focused one. */
  keydown(event) {
    if (!event.target.matches('[role="tab"]')) return;
    if (event.key === 'Delete') return oyaBrowser.closeTab(Number(event.target.closest('.tab-item').dataset.id));
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    const buttons = [...Dom.byId('tab-list').querySelectorAll('[role="tab"]')];
    event.preventDefault();
    const index = ToolNav.nextIndex(event.key, buttons.indexOf(event.target), buttons.length);
    buttons[index].click();
    buttons[index].focus();
  },
};

Dom.byId('btn-new-tab').addEventListener('click', () => oyaBrowser.newTab());
oyaBrowser.onTabsUpdated(TabStrip.update);
Dom.byId('tab-list').addEventListener('keydown', TabStrip.keydown);
Dom.byId('tab-list').addEventListener('pointerleave', TabCard.hide);
// Nothing on the strip starts the page's drag-and-drop: it would cancel a tab's own drag.
Dom.byId('tab-list').addEventListener('dragstart', (event) => event.preventDefault());
Dom.byId('tab-bar').addEventListener('pointerleave', TabStrip.release);
// The strip's room changes with the window, and when the bar first shows (tabs can arrive before browsing mode does).
if (typeof ResizeObserver === 'function') new ResizeObserver(() => TabStrip.layout()).observe(Dom.byId('tab-bar'));
