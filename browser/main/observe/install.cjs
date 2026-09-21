/**
 * Where the observer is attached: the persona's session for requests, and each
 * tab's webContents for console entries.
 *
 * Both hooks live in the browser process. `webRequest` is the same API
 * governance uses to contain egress, and `console-message` is Electron's own
 * renderer channel, so watching costs the page nothing it can observe.
 */

/** Watches every request the persona's session finishes or fails. */
function watchSession(observer, session) {
  session.webRequest.onCompleted((details) => observer.addRequest(details));
  session.webRequest.onErrorOccurred((details) =>
    observer.addRequest({ ...details, error: details.error || 'request failed' }),
  );
}

/** Watches one tab's console. */
function watchContents(observer, contents) {
  contents.on('console-message', (_event, level, message, line, sourceId) =>
    observer.addConsole({ level, message, line, sourceId }),
  );
}

module.exports = { watchSession, watchContents };
