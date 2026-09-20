/**
 * Recorded steps name their tab ('main', 'tab-1', …) rather than carry the
 * desktop's tab ids, which mean nothing to a replay.
 */

/** Tab id → recorded tab name. */
class RecordingTabNames {
  /** `steps()` is the recording's steps, whose names are taken too. */
  constructor(steps) {
    /** The recording's steps, asked when a name is chosen. */
    this.steps = steps;
    /** Tab id → name. */
    this.names = new Map();
  }

  /** The tab's name, choosing the first free `tab-N` for a new one. */
  recordingTab(id) {
    if (!this.names.has(id)) this.names.set(id, this.freeName());
    return this.names.get(id);
  }

  /** The first `tab-N` no tab and no step uses. */
  freeName() {
    const used = new Set([...this.names.values(), ...this.steps().map((s) => s.tab)]);
    let n = 1;
    while (used.has('tab-' + n)) n++;
    return 'tab-' + n;
  }

  /** Names a tab explicitly. */
  set(id, name) {
    this.names.set(id, name);
  }

  /** How many tabs are named. */
  get size() {
    return this.names.size;
  }

  /** Forgets every name. */
  clear() {
    this.names.clear();
  }
}

module.exports = { RecordingTabNames };
