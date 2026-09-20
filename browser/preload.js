/**
 * The shell's bridge to the main process: window.oyaBrowser, one method per IPC
 * channel. Only the shell window loads it; tabs and the control shield get no
 * preload, so pages cannot reach any of this.
 */
/* global window */
const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('oyaBrowser', {
  navigate: (url) => ipcRenderer.invoke('navigate', url),
  goBack: () => ipcRenderer.invoke('go-back'),
  goForward: () => ipcRenderer.invoke('go-forward'),
  reload: () => ipcRenderer.invoke('reload'),
  getConfig: () => ipcRenderer.invoke('get-config'),
  saveConfig: (config) => ipcRenderer.invoke('save-config', config),
  getStatus: () => ipcRenderer.invoke('get-status'),
  getControlState: () => ipcRenderer.invoke('get-control-state'),
  changeControl: (action) => ipcRenderer.invoke('change-control', action),
  onControlState: (cb) => ipcRenderer.on('control-state', (_event, state) => cb(state)),
  onPageBackdrop: (cb) => ipcRenderer.on('page-backdrop', (_event, state) => cb(state)),
  backdropReady: (token) => ipcRenderer.invoke('backdrop-ready', token),
  enterBrowsing: () => ipcRenderer.invoke('enter-browsing'),
  showOverlay: (name) => ipcRenderer.invoke('show-overlay', name),
  hideOverlay: (name) => ipcRenderer.invoke('hide-overlay', name),
  getUiPreferences: () => ipcRenderer.invoke('get-ui-preferences'),
  saveUiPreferences: (preferences) => ipcRenderer.invoke('save-ui-preferences', preferences),
  exportPlaywright: (payload) => ipcRenderer.invoke('export-playwright', payload),
  confirmDiscardRecording: () => ipcRenderer.invoke('confirm-discard-recording'),
  onShellLayout: (cb) => ipcRenderer.on('shell-layout', (_e, layout) => cb(layout)),
  onShellCommand: (cb) => ipcRenderer.on('shell-command', (_e, command) => cb(command)),
  onShellAppearance: (cb) => ipcRenderer.on('shell-appearance', (_e, dark) => cb(dark)),
  toggleDevPanel: () =>
    ipcRenderer.invoke('toggle-dev-panel', window.matchMedia('(prefers-reduced-motion: reduce)').matches),
  // Tabs
  newTab: (url) => ipcRenderer.invoke('new-tab', url),
  closeTab: (id) => ipcRenderer.invoke('close-tab', id),
  activateTab: (id) => ipcRenderer.invoke('activate-tab', id),
  workspace: (command) => ipcRenderer.invoke('workspace', command),
  onWorkspace: (cb) => ipcRenderer.on('workspace-state', (_event, state) => cb(state)),
  // Recording
  startRecording: () => ipcRenderer.invoke('start-recording'),
  stopRecording: () => ipcRenderer.invoke('stop-recording'),
  clearRecording: () => ipcRenderer.invoke('clear-recording'),
  saveRecording: (name, description) => ipcRenderer.invoke('save-recording', name, description),
  onRecordedSteps: (cb) => ipcRenderer.on('recorded-steps', (e, state) => cb(state)),
  // Events
  onUrlChanged: (cb) => ipcRenderer.on('url-changed', (e, url) => cb(url)),
  onTitleChanged: (cb) => ipcRenderer.on('title-changed', (e, title) => cb(title)),
  onWsStatus: (cb) => ipcRenderer.on('ws-status', (e, status) => cb(status)),
  onModeChanged: (cb) => ipcRenderer.on('mode-changed', (e, mode) => cb(mode)),
  onDevLog: (cb) => ipcRenderer.on('dev-log', (e, entry) => cb(entry)),
  onDevPanelState: (cb) => ipcRenderer.on('dev-panel-state', (e, open) => cb(open)),
  onInspectResult: (cb) => ipcRenderer.on('inspect-result', (e, result) => cb(result)),
  onViewSource: (cb) => ipcRenderer.on('view-source', (e, data) => cb(data)),
  onTabsUpdated: (cb) => ipcRenderer.on('tabs-updated', (e, tabs) => cb(tabs)),
  // Dev panel
  resizeDevPanel: (width) => ipcRenderer.invoke('resize-dev-panel', width),
  getPageSource: () => ipcRenderer.invoke('get-page-source'),
  renderPage: (analysis, format) => ipcRenderer.invoke('render-page', analysis, format),
  devAction: (action, params) => ipcRenderer.invoke('dev-action', action, params),
  sendChat: (messages) => ipcRenderer.invoke('send-chat', messages),
  saveChatPlaybook: (name) => ipcRenderer.invoke('save-chat-playbook', name),
  getFingerprint: () => ipcRenderer.invoke('get-fingerprint'),
  saveProfile: () => ipcRenderer.invoke('save-profile'),
  onProfileSaved: (cb) => ipcRenderer.on('profile-saved', (e, state) => cb(state)),
  onFingerprintChanged: (cb) => ipcRenderer.on('fingerprint-changed', (e, fp) => cb(fp)),
  // Updates
  getVersion: () => ipcRenderer.invoke('get-version'),
  getUpdateStatus: () => ipcRenderer.invoke('get-update-status'),
  checkForUpdates: () => ipcRenderer.invoke('check-for-updates'),
  installUpdate: () => ipcRenderer.invoke('install-update'),
  onUpdateStatus: (cb) => ipcRenderer.on('update-status', (e, status) => cb(status)),
});
