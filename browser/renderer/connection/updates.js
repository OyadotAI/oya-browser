/**
 * Updates.
 *
 * One control does three jobs: it is the version label, the "check now"
 * button, and, once an update is staged, the restart button. The update
 * installs on quit regardless, so nothing here ever blocks browsing.
 */
/* global oyaBrowser, Dom */
/* exported Updates */

/** The update pill. */
const Updates = {
  /** An update is staged: a click restarts. */
  ready: false,

  /** The pill's text for each update state (`v` is "v1.2.3" or ''). */
  LABELS: {
    checking: () => 'Checking…',
    available: ({ version }) => `Update ${version} available`,
    downloading: ({ version, percent }) => `Downloading ${version}… ${percent ?? 0}%`,
    ready: ({ version }) => `Update ${version} ready, Restart`,
    error: ({ v }) => `${v}, check failed`,
    unsupported: ({ v }) => v,
  },

  /** The pill's text; any other state means up to date. */
  label(status) {
    const v = status.current ? 'v' + status.current : '';
    if (Object.hasOwn(Updates.LABELS, status.state)) return Updates.LABELS[status.state]({ ...status, v });
    return `${v}, up to date`;
  },

  /** The pill's tooltip. */
  title(state) {
    if (Updates.ready) return 'Restart to finish updating';
    return state === 'unsupported' ? 'Updates apply to installed builds' : 'Click to check for updates';
  },

  /** Shows an update status on the pill and the version in settings. */
  render(status) {
    const { state, current } = status;
    Updates.ready = state === 'ready';
    Dom.byId('app-version').textContent = current ? 'Oya Browser · v' + current : 'Oya Browser';
    Updates.pill(Dom.byId('update-pill'), status);
  },

  /** The toolbar pill: shown while there is something to say, highlighted while an update is coming. */
  pill(btn, status) {
    const { state } = status;
    btn.hidden = !['available', 'downloading', 'ready', 'error'].includes(state);
    btn.classList.toggle('attention', state === 'available' || state === 'downloading' || Updates.ready);
    btn.disabled = state === 'checking' || state === 'downloading';
    btn.textContent = Updates.label(status);
    btn.title = Updates.title(state);
  },

  /** Restarts into a staged update, or checks for one. */
  async click() {
    if (Updates.ready) {
      Dom.byId('update-pill').textContent = 'Restarting…';
      oyaBrowser.installUpdate();
      return;
    }
    Updates.render({ state: 'checking' });
    Updates.render(await oyaBrowser.checkForUpdates());
  },
};

oyaBrowser
  .getUpdateStatus()
  .then(Updates.render)
  .catch(() => {});
oyaBrowser.onUpdateStatus(Updates.render);
Dom.byId('update-pill').addEventListener('click', Updates.click);
