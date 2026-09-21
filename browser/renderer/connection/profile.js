/**
 * The profile section of the connection dialog: what the profile syncs, and
 * saving it to the server on demand.
 */
/* global oyaBrowser, Dom, RendererConstants, ConnectionStatus */
/* exported ProfileSave */

/** Saving the browser profile. */
const ProfileSave = {
  /** Gives the button back if the save is never confirmed. */
  timer: undefined,

  /** Describes the profile for the current connection. */
  onStatus(s) {
    Dom.byId('fp-content').textContent = s.connected
      ? 'Profile · ' + (s.profileName || 'Default') + ' · Account sessions sync automatically'
      : 'Offline · Your account sessions remain on this desktop';
    Dom.byId('save-profile').disabled = !s.connected;
  },

  /** Asks the main process to save; the confirmation arrives separately. */
  async save() {
    const [button, status] = [Dom.byId('save-profile'), Dom.byId('profile-save-status')];
    button.disabled = true;
    status.textContent = 'Saving…';
    ProfileSave.awaitConfirmation();
    await oyaBrowser.saveProfile().catch(ProfileSave.refused);
  },

  /** The save was refused outright: no confirmation is coming. */
  refused(error) {
    clearTimeout(ProfileSave.timer);
    ProfileSave.finish(error.message);
  },

  /** The confirmation arrives separately; without it in time, say so. */
  awaitConfirmation() {
    clearTimeout(ProfileSave.timer);
    const unconfirmed = () => ProfileSave.finish('Save not confirmed. Try again.');
    ProfileSave.timer = setTimeout(unconfirmed, RendererConstants.PROFILE_SAVE_TIMEOUT_MS);
  },

  /** Shows the outcome and gives the button back. */
  finish(text) {
    Dom.byId('profile-save-status').textContent = text;
    Dom.byId('save-profile').disabled = false;
  },

  /** The main process confirmed a save. */
  saved(state) {
    clearTimeout(ProfileSave.timer);
    Dom.byId('save-profile').disabled = false;
    Dom.byId('profile-save-status').textContent = state.error || 'Saved · ' + (state.sites?.length || 0) + ' sites';
  },
};

oyaBrowser.onWsStatus(ProfileSave.onStatus);
Dom.byId('save-profile').addEventListener('click', ProfileSave.save);
oyaBrowser.onProfileSaved(ProfileSave.saved);

// ── Init ──
oyaBrowser.getConfig().then(ConnectionStatus.loadConfig);
oyaBrowser.getStatus().then(ConnectionStatus.loadStatus);
