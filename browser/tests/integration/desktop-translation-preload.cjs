/** Restricted bridge for the disposable translation fixture; no production page receives this preload. */
const { contextBridge, ipcRenderer } = require('electron');
contextBridge.exposeInMainWorld('translationFixture', {
  call: (name, args) => ipcRenderer.invoke('translation-fixture', name, args),
  onTabs: (listener) => ipcRenderer.on('translation-tabs', (_event, tabs) => listener(tabs)),
  onBackdrop: (listener) => ipcRenderer.on('page-backdrop', (_event, payload) => listener(payload)),
});
