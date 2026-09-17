/* Desktop shell. IPC is provided only to this renderer, never to visited pages. */
(() => {
  const paths = {
    back: 'm14 5-7 7 7 7M7 12h13', forward: 'm10 5 7 7-7 7M4 12h13',
    reload: 'M20 7v5h-5M19 12a7 7 0 1 1-2-5l3 3', plus: 'M12 5v14M5 12h14',
    close: 'm6 6 12 12M6 18 18 6', menu: 'M5 6h14M5 12h14M5 18h14',
    globe: 'M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0ZM3 12h18M12 3c-5 5-5 13 0 18 5-5 5-13 0-18Z',
    record: 'M19 12a7 7 0 1 1-14 0 7 7 0 0 1 14 0Z', stop: 'M6 6h12v12H6Z',
    panel: 'M3 4h18v16H3ZM15 4v16', send: 'm5 12 7-7 7 7M12 5v15',
    spark: 'm12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5Z',
    chevron: 'm9 5 7 7-7 7', profile: 'M16 8a4 4 0 1 1-8 0 4 4 0 0 1 8 0ZM4 21v-2a8 8 0 0 1 16 0v2',
    check: 'm5 12 4 4L19 6', code: 'm8 7-5 5 5 5m8-10 5 5-5 5',
    scan: 'M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5M7 8h10M7 12h10M7 16h6',
    camera: 'M3 7h4l2-3h6l2 3h4v13H3ZM16 13a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z',
    down: 'M12 4v16m-6-6 6 6 6-6', up: 'M12 20V4m-6 6 6-6 6 6',
  };
  window.shellIcon = name => `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${paths[name] || paths.code}"/></svg>`;
  document.querySelectorAll('[data-icon]').forEach(el => { el.innerHTML = shellIcon(el.dataset.icon); });
  document.querySelectorAll('.action-icon').forEach(el => {
    const name = { analyze: 'scan', screenshot: 'camera', reload: 'reload', 'scroll-down': 'down', 'scroll-up': 'up', 'list-tabs': 'panel' }[el.closest('[data-action]')?.dataset.action];
    el.innerHTML = shellIcon(name || 'code');
  });
  document.querySelectorAll('.action-field').forEach(el => {
    if (!el.hasAttribute('aria-label')) el.setAttribute('aria-label', el.placeholder || 'Action parameter');
  });
  document.getElementById('source-refresh').textContent = 'Refresh';

  const root = document.documentElement;
  const media = matchMedia('(prefers-color-scheme: dark)');
  let theme = 'system';
  let systemDark = media.matches;
  function applyTheme() { root.dataset.theme = theme === 'system' ? (systemDark ? 'dark' : 'light') : theme; }
  oyaBrowser.onShellAppearance(dark => { systemDark = dark; applyTheme(); });
  const preference = document.getElementById('theme-preference');
  preference.addEventListener('change', () => {
    theme = preference.value; applyTheme(); oyaBrowser.saveUiPreferences({ theme });
  });
  applyTheme();
  oyaBrowser.getUiPreferences().then(value => {
    theme = ['system', 'light', 'dark'].includes(value.theme) ? value.theme : 'system';
    if (typeof value.systemDark === 'boolean') systemDark = value.systemDark;
    preference.value = theme; root.dataset.platform = value.platform; applyTheme();
    if (['record', 'chat', 'actions', 'network', 'source'].includes(value.pane)) showDevPane(value.pane);
  });
  oyaBrowser.onShellLayout(layout => {
    root.style.setProperty('--panel-reveal', layout.reveal + 'px');
    document.getElementById('dev-panel').classList.toggle('open', layout.progress > 0);
    root.dataset.panelMoving = String(layout.progress > 0 && layout.progress < 1);
    root.style.setProperty('--panel-width', layout.panelWidth + 'px');
    root.style.setProperty('--panel-height', layout.panelHeight + 'px');
    root.style.setProperty('--chrome-height', layout.chromeHeight + 'px');
    const handle = document.querySelector('.dev-panel-resize');
    handle.setAttribute('aria-valuenow', layout.panelWidth);
    handle.setAttribute('aria-valuemax', Math.min(560, innerWidth - 480));
  });
  const resizeHandle = document.querySelector('.dev-panel-resize');
  resizeHandle.tabIndex = 0; resizeHandle.setAttribute('role', 'separator');
  resizeHandle.setAttribute('aria-label', 'Resize workspace tools');
  resizeHandle.setAttribute('aria-orientation', 'vertical'); resizeHandle.setAttribute('aria-valuemin', '320');
  resizeHandle.addEventListener('keydown', event => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const width = document.getElementById('dev-panel').getBoundingClientRect().width;
    oyaBrowser.resizeDevPanel(event.key === 'Home' ? 320 : event.key === 'End' ? 560 : width + (event.key === 'ArrowLeft' ? 16 : -16));
  });

  window.syncToolNavigation = pane => {
    const inspect = ['actions', 'network', 'source'].includes(pane);
    document.getElementById('inspect-nav').hidden = !inspect;
    document.querySelectorAll('.dev-tab').forEach(button => {
      const active = button.dataset.pane === pane || (button.id === 'inspect-tab' && inspect);
      button.classList.toggle('active', active);
      button.setAttribute('aria-selected', String(active));
      button.tabIndex = active ? 0 : -1;
      button.id ||= 'tool-tab-' + button.dataset.pane;
      button.setAttribute('aria-controls', 'pane-' + (button.id === 'inspect-tab' && inspect ? pane : button.dataset.pane));
    });
    document.querySelectorAll('.dev-pane').forEach(panel => {
      panel.setAttribute('role', 'tabpanel');
      const name = panel.id.slice('pane-'.length);
      panel.setAttribute('aria-labelledby', ['actions', 'network', 'source'].includes(name) ? 'inspect-tab' : 'tool-tab-' + name);
    });
    document.querySelectorAll('[data-inspect]').forEach(button => {
      button.classList.toggle('active', button.dataset.inspect === pane);
      button.setAttribute('aria-pressed', String(button.dataset.inspect === pane));
    });
  };
  syncToolNavigation(activeDevPane);
  document.querySelectorAll('[data-inspect]').forEach(button => button.addEventListener('click', () => showDevPane(button.dataset.inspect)));
  document.querySelector('.dev-panel-header').addEventListener('keydown', event => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    const items = [...document.querySelectorAll('.dev-tab')];
    const current = items.indexOf(document.activeElement);
    if (current < 0) return;
    event.preventDefault();
    const index = event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : (current + (event.key === 'ArrowRight' ? 1 : items.length - 1)) % items.length;
    items[index].click(); items[index].focus();
  });
  document.getElementById('tools-close').addEventListener('click', () => { oyaBrowser.toggleDevPanel(); document.getElementById('btn-dev').focus(); });
  oyaBrowser.onDevPanelState(open => document.getElementById('btn-dev').setAttribute('aria-expanded', String(open)));

  let lastFocus, dialogClose, activeDialog;
  window.activateDialog = (element, close) => {
    lastFocus = document.activeElement; activeDialog = element; dialogClose = close;
  };
  window.deactivateDialog = () => {
    activeDialog = null; dialogClose = null;
    if (lastFocus?.isConnected) lastFocus.focus();
  };
  document.addEventListener('keydown', event => {
    if (!activeDialog) return;
    if (event.key === 'Escape') { event.preventDefault(); dialogClose?.(); return; }
    if (event.key !== 'Tab') return;
    const items = [...activeDialog.querySelectorAll('button:not(:disabled), input, select, textarea, [tabindex="0"]')].filter(el => el.getClientRects().length);
    const first = items[0], last = items.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  });

  const overlay = document.getElementById('shell-overlay');
  const backdrop = document.getElementById('page-backdrop');
  let backdropToken;
  oyaBrowser.onPageBackdrop(async value => {
    backdropToken = value?.token;
    if (!value) { backdrop.hidden = true; backdrop.removeAttribute('src'); return; }
    const { bounds } = value;
    Object.assign(backdrop.style, { left: bounds.x + 'px', top: bounds.y + 'px', width: bounds.width + 'px', height: bounds.height + 'px' });
    backdrop.src = value.image;
    await backdrop.decode().catch(() => {});
    if (backdropToken !== value.token) return;
    backdrop.hidden = false;
    await new Promise(resolve => requestAnimationFrame(resolve));
    oyaBrowser.backdropReady(value.token);
  });
  const search = document.getElementById('command-search');
  window.closeShellDialog = () => {
    overlay.hidden = true; oyaBrowser.hideOverlay('shell'); deactivateDialog();
  };
  async function openShellDialog(profile = false) {
    await oyaBrowser.showOverlay('shell');
    overlay.hidden = false;
    document.getElementById('commands-section').hidden = profile;
    document.getElementById('profile-section').hidden = !profile;
    document.getElementById('shell-dialog-title').textContent = profile ? 'Connection & profile' : 'Commands';
    overlay.setAttribute('aria-label', profile ? 'Connection and profile' : 'Commands and settings');
    activateDialog(overlay, closeShellDialog);
    if (profile) document.getElementById('connection-edit').focus();
    else { search.value = ''; renderCommands(); search.focus(); }
  }
  document.getElementById('shell-dialog-close').addEventListener('click', closeShellDialog);
  overlay.addEventListener('click', event => { if (event.target === overlay) closeShellDialog(); });
  document.getElementById('conn-pill').addEventListener('click', () => openShellDialog(true));
  document.getElementById('btn-commands').addEventListener('click', () => openShellDialog());
  let currentBrowserId = '';
  oyaBrowser.getConfig().then(value => {
    document.getElementById('profile-server').textContent = value.serverUrl || 'Not configured';
    document.getElementById('profile-title').textContent = value.browserName || 'This browser';
  });
  const updateProfile = value => {
    currentBrowserId = value.browserId || '';
    document.getElementById('profile-browser-id').textContent = currentBrowserId || 'Not connected';
    document.getElementById('copy-browser-id').disabled = !currentBrowserId;
  };
  oyaBrowser.onWsStatus(updateProfile);
  oyaBrowser.getStatus().then(updateProfile);
  document.getElementById('copy-browser-id').addEventListener('click', async () => {
    try { await navigator.clipboard.writeText(currentBrowserId); document.getElementById('profile-save-status').textContent = 'Browser ID copied.'; }
    catch { document.getElementById('profile-save-status').textContent = 'Could not copy. Select the browser ID to copy it manually.'; }
  });
  document.getElementById('copy-server').addEventListener('click', async () => {
    try { await navigator.clipboard.writeText(document.getElementById('profile-server').textContent); document.getElementById('profile-save-status').textContent = 'Server address copied.'; }
    catch { document.getElementById('profile-save-status').textContent = 'Could not copy. Select the server address to copy it manually.'; }
  });
  const modifier = navigator.platform.includes('Mac') ? '⌘' : 'Ctrl';
  const commands = [
    ['Focus address bar', modifier + ' L', () => { urlBar.focus(); urlBar.select(); }],
    ['New tab', modifier + ' T', () => oyaBrowser.newTab()],
    ['Record a workflow', modifier + ' ⇧ R', () => document.getElementById('btn-record').click()],
    ['Toggle workspace tools', modifier + ' ⇧ D', () => oyaBrowser.toggleDevPanel()],
    ['Ask Oya', '', () => { if (!devOpen) oyaBrowser.toggleDevPanel(); showDevPane('chat'); document.getElementById('chat-input').focus(); }],
    ['Inspect this page', '', () => { if (!devOpen) oyaBrowser.toggleDevPanel(); showDevPane('actions'); }],
    ['Connection and profile', '', () => openShellDialog(true)],
    ['Check for updates', '', async () => { renderUpdate(await oyaBrowser.checkForUpdates()); }],
  ];
  function renderCommands() {
    const list = document.getElementById('command-list'); list.replaceChildren();
    for (const [label, shortcut, run] of commands.filter(([label]) => label.toLowerCase().includes(search.value.toLowerCase()))) {
      const button = document.createElement('button'); button.className = 'command';
      const text = document.createElement('span'); text.textContent = label;
      const key = document.createElement('kbd'); key.textContent = shortcut;
      button.append(text, key);
      button.addEventListener('click', async () => { closeShellDialog(); await run(); }); list.append(button);
    }
    if (!list.children.length) { const empty = document.createElement('p'); empty.textContent = 'No matching commands.'; list.append(empty); }
  }
  search.addEventListener('input', renderCommands);
  search.addEventListener('keydown', event => {
    if (event.key === 'ArrowDown') { event.preventDefault(); document.querySelector('.command')?.focus(); }
    if (event.key === 'Enter') document.querySelector('.command')?.click();
  });
  document.getElementById('command-list').addEventListener('keydown', event => {
    if (!['ArrowDown', 'ArrowUp'].includes(event.key)) return;
    event.preventDefault(); const buttons = [...document.querySelectorAll('.command')];
    const index = buttons.indexOf(document.activeElement);
    buttons[(index + (event.key === 'ArrowDown' ? 1 : buttons.length - 1)) % buttons.length]?.focus();
  });
  oyaBrowser.onShellCommand(command => {
    if (command === 'address') { urlBar.focus(); urlBar.select(); }
    if (command === 'commands') openShellDialog();
    if (command === 'record') document.getElementById('btn-record').click();
    if (command === 'tools') oyaBrowser.toggleDevPanel();
  });

  document.getElementById('btn-new-tab').addEventListener('click', () => oyaBrowser.newTab());
  const tabList = document.getElementById('tab-list');
  oyaBrowser.onTabsUpdated(tabs => {
    for (const item of [...tabList.children]) if (!tabs.some(tab => String(tab.id) === item.dataset.id)) item.remove();
    for (const tab of tabs) {
      let item = [...tabList.children].find(node => node.dataset.id === String(tab.id));
      if (!item) {
        item = document.createElement('div'); item.className = 'tab-item'; item.dataset.id = tab.id;
        const loading = document.createElement('span'); loading.className = 'tab-loading'; loading.setAttribute('aria-hidden', 'true');
        const title = document.createElement('button'); title.className = 'tab-title'; title.setAttribute('role', 'tab');
        title.addEventListener('click', () => oyaBrowser.activateTab(tab.id));
        const close = document.createElement('button'); close.className = 'tab-close'; close.innerHTML = shellIcon('close');
        close.addEventListener('click', () => oyaBrowser.closeTab(tab.id));
        item.append(loading, title, close); tabList.append(item);
      }
      item.classList.toggle('active', tab.active); item.title = tab.url || tab.title;
      item.querySelector('.tab-loading').hidden = !tab.loading;
      const title = item.querySelector('.tab-title');
      title.textContent = tab.title || 'New tab'; title.setAttribute('aria-selected', String(tab.active)); title.tabIndex = tab.active ? 0 : -1;
      item.querySelector('.tab-close').setAttribute('aria-label', 'Close ' + (tab.title || 'tab'));
      if (tab.active) {
        document.getElementById('navigation-progress').hidden = !tab.loading;
        const status = document.getElementById('navigation-status');
        status.textContent = tab.loadError ? 'Load failed' : tab.loading ? 'Loading…' : '';
        status.title = tab.loadError || '';
        status.setAttribute('aria-label', tab.loadError || status.textContent);
        const reload = document.getElementById('btn-reload');
        reload.innerHTML = shellIcon(tab.loading ? 'close' : 'reload');
        reload.setAttribute('aria-label', tab.loading ? 'Stop loading' : 'Reload page');
        reload.title = tab.loading ? 'Stop loading' : 'Reload page';
        document.getElementById('url-bar').setAttribute('aria-busy', String(tab.loading));
        document.getElementById('btn-back').disabled = !tab.canGoBack;
        document.getElementById('btn-forward').disabled = !tab.canGoForward;
        // Scroll only the tab strip, never the entire renderer or open tools.
        if (item.offsetLeft < tabList.scrollLeft) tabList.scrollLeft = item.offsetLeft;
        else if (item.offsetLeft + item.offsetWidth > tabList.scrollLeft + tabList.clientWidth) tabList.scrollLeft = item.offsetLeft + item.offsetWidth - tabList.clientWidth;
      }
    }
  });
  tabList.addEventListener('keydown', event => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key) || !event.target.matches('[role="tab"]')) return;
    const buttons = [...tabList.querySelectorAll('[role="tab"]')]; const current = buttons.indexOf(event.target);
    event.preventDefault(); const index = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : (current + (event.key === 'ArrowRight' ? 1 : buttons.length - 1)) % buttons.length;
    buttons[index].click(); buttons[index].focus();
  });
})();
