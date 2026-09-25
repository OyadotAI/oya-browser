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
  openConsole: (serverUrl) => ipcRenderer.invoke('open-console', serverUrl),
  signOut: () => ipcRenderer.invoke('sign-out'),
  importSources: () => ipcRenderer.invoke('import-sources'),
  reimportBrowser: (sourceId) => ipcRenderer.invoke('reimport-browser', sourceId),
  onMirrorStatus: (cb) => ipcRenderer.on('mirror-status', (_event, status) => cb(status)),
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
  saveRecording: (name, description) => ipcRenderer.invoke('save-recording', name, description),
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
  sendChat: (messages, data) => ipcRenderer.invoke('send-chat', messages, data),
  stopChat: () => ipcRenderer.invoke('stop-chat'),
  modelStatus: () => ipcRenderer.invoke('model-status'),
  saveModelKey: (choice) => ipcRenderer.invoke('save-model-key', choice),
  listPersonas: () => ipcRenderer.invoke('list-personas'),
  // Routines
  listRoutines: () => ipcRenderer.invoke('list-routines'),
  saveRoutine: (routine) => ipcRenderer.invoke('save-routine', routine),
  deleteRoutine: (id) => ipcRenderer.invoke('delete-routine', id),
  runRoutineNow: (id) => ipcRenderer.invoke('run-routine-now', id),
  setRoutineEnabled: (id, enabled) => ipcRenderer.invoke('set-routine-enabled', id, enabled),
  clearRoutineHistory: (id) => ipcRenderer.invoke('clear-routine-history', id),
  stopRoutine: (id) => ipcRenderer.invoke('stop-routine', id),
  onRoutinesChanged: (cb) => ipcRenderer.on('routines-changed', (e, state) => cb(state)),
  saveChatPlaybook: (name) => ipcRenderer.invoke('save-chat-playbook', name),
  getFingerprint: () => ipcRenderer.invoke('get-fingerprint'),
  saveProfile: () => ipcRenderer.invoke('save-profile'),
  onProfileSaved: (cb) => ipcRenderer.on('profile-saved', (e, state) => cb(state)),
  onSettingsChanged: (cb) => ipcRenderer.on('settings-changed', (e, change) => cb(change)),
  onFingerprintChanged: (cb) => ipcRenderer.on('fingerprint-changed', (e, fp) => cb(fp)),
  // Updates
  getVersion: () => ipcRenderer.invoke('get-version'),
  getUpdateStatus: () => ipcRenderer.invoke('get-update-status'),
  checkForUpdates: () => ipcRenderer.invoke('check-for-updates'),
  installUpdate: () => ipcRenderer.invoke('install-update'),
  onUpdateStatus: (cb) => ipcRenderer.on('update-status', (e, status) => cb(status)),
});
