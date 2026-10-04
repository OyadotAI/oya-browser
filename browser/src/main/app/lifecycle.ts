/**
 * Shut-down: what must finish before the app quits. A recording is finished
 * first, a validation is marked interrupted, and the cookie jar is written to
 * disk.
 */
import type { Event } from 'electron';
import type { AppServices } from './services.ts';

/** The services quitting uses. */
type Deps = Pick<AppServices, 'electron' | 'persona' | 'cookies' | 'socket' | 'recorder' | 'workspace' | 'layout'>;

/** The workspace, as far as an interrupted validation goes. */
type RunningWorkspace = NonNullable<AppServices['workspace']>;

/** Marks a running validation interrupted, and ends its session. */
function interruptValidation(workspace: RunningWorkspace): void {
  const error = 'Oya closed during validation. Check the website before retrying.';
  workspace.receive({ type: 'finished', status: 'interrupted', error });
  workspace.session?.dispose();
}

/** Quitting: a recording is finished first, a validation is marked interrupted, the jar is written to disk. */
export class Lifecycle {
  /** Set once a quit has waited for a recording, so the second quit goes through. */
  private finishingQuit = false;
  /** Set once the jar has been written to disk for this quit, so the next quit goes through. */
  private jarFlushed = false;
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Writes the jar to disk once; later calls answer at once. The updater calls it before restarting. */
  flushJar(): Promise<void> {
    if (this.jarFlushed) return Promise.resolve();
    this.jarFlushed = true;
    return this.deps.persona.flushJar();
  }

  /** Installs the quit handlers. */
  install(): void {
    const { app } = this.deps.electron;
    app.on('window-all-closed', () => {
      // A login made seconds ago is still queued: send it while the socket is up.
      this.deps.cookies.flushCookieChanges();
      this.deps.socket.disconnect();
      app.quit();
    });
    app.on('before-quit', (event) => this.beforeQuit(event));
  }

  /** Runs before the app quits. */
  private beforeQuit(event: Event): void {
    if (this.deps.recorder.recording && !this.finishingQuit) return this.finishRecordingFirst(event);
    if (this.deps.workspace?.busy()) interruptValidation(this.deps.workspace);
    this.deps.cookies.flushCookieChanges();
    this.deps.layout.flush();
    if (!this.jarFlushed) this.holdForJar(event);
  }

  /** Holds the quit until the cookies and storage are on disk, then quits again. */
  private holdForJar(event: Event): void {
    event.preventDefault();
    const quit = (): void => this.deps.electron.app.quit();
    // A jar that cannot be written must not keep the app from quitting.
    this.flushJar().then(quit, quit);
  }

  /** Holds the quit until the recording is stopped and saved. */
  private finishRecordingFirst(event: Event): void {
    event.preventDefault();
    this.finishingQuit = true;
    const recorder = this.deps.recorder;
    recorder.queueRecording(() => recorder.stopRecording()).finally(() => this.deps.electron.app.quit());
  }
}
