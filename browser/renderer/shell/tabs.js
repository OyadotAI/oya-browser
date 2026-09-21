/**
 * The tab strip: one item per open page, kept in step with the main process,
 * plus the toolbar state that follows the active tab.
 */
/* global oyaBrowser, Dom, ShellIcons, ToolNav */
/* exported TabStrip */

/** The tab strip. */
const TabStrip = {
  /** Brings the strip in line with `tabs`: removes closed ones, adds and updates the rest. */
  update(tabs) {
    const tabList = Dom.byId('tab-list');
    for (const item of [...tabList.children])
      if (!tabs.some((tab) => String(tab.id) === item.dataset.id)) item.remove();
    for (const tab of tabs) {
      const item = [...tabList.children].find((node) => node.dataset.id === String(tab.id)) || TabStrip.create(tab);
      TabStrip.render(item, tab);
      if (tab.active) TabStrip.activate(item, tab);
    }
  },

  /** A new tab item: loading indicator, title button and close button. */
  create(tab) {
    const item = Dom.node('div', null, 'tab-item');
    item.dataset.id = tab.id;
    const loading = Dom.node('span', null, 'tab-loading');
    loading.setAttribute('aria-hidden', 'true');
    item.append(loading, TabStrip.titleButton(tab), TabStrip.closeButton(tab));
    Dom.byId('tab-list').append(item);
    return item;
  },

  /** A tab's title: a tab button that activates it. */
  titleButton(tab) {
    const title = Dom.node('button', null, 'tab-title');
    title.setAttribute('role', 'tab');
    title.addEventListener('click', () => oyaBrowser.activateTab(tab.id));
    return title;
  },

  /** A tab's close button. */
  closeButton(tab) {
    const close = Dom.node('button', null, 'tab-close');
    close.innerHTML = ShellIcons.icon('close');
    close.addEventListener('click', () => oyaBrowser.closeTab(tab.id));
    return close;
  },

  /** A tab item's title, state and labels. */
  render(item, tab) {
    item.classList.toggle('active', tab.active);
    item.title = tab.url || tab.title;
    item.querySelector('.tab-loading').hidden = !tab.loading;
    const title = item.querySelector('.tab-title');
    title.textContent = tab.title || 'New tab';
    title.setAttribute('aria-selected', String(tab.active));
    title.tabIndex = tab.active ? 0 : -1;
    item.querySelector('.tab-close').setAttribute('aria-label', 'Close ' + (tab.title || 'tab'));
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

  /** Arrow keys, Home and End move between tabs. */
  keydown(event) {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key) || !event.target.matches('[role="tab"]'))
      return;
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
