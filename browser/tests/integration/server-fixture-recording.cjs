/** Native server recording regressions use the production recorder and isolated preload, never Runtime bindings. */
const fs = require('node:fs');
const path = require('node:path');
const { Recorder } = require('../../src/main/recording/recorder.ts');
/** Compose one fixture-owned recording surface with the production lifecycle and step normalization. */
module.exports = function fixtureRecording(window) {
  const view = { webContents: window.webContents };
  const deps = {
    tabs: { activeTabId: 1, list: [{ id: 1, view }], getActiveView: () => view },
    analyzerScript: fs.readFileSync(path.resolve(__dirname, '../../scripts/analyzer.js'), 'utf8'),
  };
  deps.recorder = new Recorder(deps);
  return (mode) => deps.recorder.queueRecording(() => deps.recorder.remote(mode));
};
