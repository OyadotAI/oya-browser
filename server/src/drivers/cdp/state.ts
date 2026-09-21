/**
 * What a CDP driver knows about its browser: the connection, the attached
 * target, the persona it applies, and the live-view, dialog and recording
 * state. CDPDriver adds the behaviour on top.
 */
import { randomBytes } from 'crypto';
import { createRequire } from 'module';
import { CDP_CAPABILITIES } from './actions.ts';
import { WORLD_NAME_BYTES } from './constants.ts';

const require = createRequire(import.meta.url);
const { LoginState } = require('../../../../browser/login-state.js');

/** The state one CDP driver carries between commands. */
export class CDPDriverState {
  /** Unsubscribes the screencast frame listener. */
  declare offScreencast: any;
  /** Called when the CDP socket closes. */
  declare onClose: any;
  /** Whether a live-view recording is in progress. */
  declare recording: any;
  /** Accept-Language derived from the persona's languages. */
  declare acceptLanguage: any;
  /** Whether the analyzer is loaded in the current target; reset on attach. */
  declare analyzerLoaded: any;
  /** The CDP connection to the browser. */
  declare conn: any;
  /** Descriptions of dialogs seen during the current command. */
  declare dialogNotes: any;
  /** Resolvers waiting to hear that a dialog is being held open. */
  declare dialogWaiters: any;
  /** Set once the dialog listeners are registered. */
  declare dialogsWatched: any;
  /** The persona fingerprint to apply, or null. */
  declare fingerprint: any;
  /** Saved login to restore: cookies plus localStorage by origin. */
  declare login: any;
  /** Restores and captures localStorage for the saved login. */
  declare loginState: any;
  /** A confirm or prompt dialog waiting for an answer. */
  declare pendingDialog: any;
  /** The persona applier bound to the current connection. */
  declare personaApply: any;
  /** The provider name (anchor, browserbase, steel, …) or 'cdp'. */
  declare provider: any;
  /** Streams recorded steps out of the page while recording. */
  declare recordChannel: any;
  /** Steps recorded so far, capped at 500. */
  declare recorded: any;
  /** Ids of recorded steps, so a step is never collected twice. */
  declare recordedIds: Set<any>;
  /** Names of secrets the recording referenced. */
  declare recordedSecrets: Set<any>;
  /** Screenshot timer that fills the live view while the page is idle. */
  declare screencastFill: ReturnType<typeof setTimeout>;
  /** Whether the live-view screencast is running. */
  declare screencasting: any;
  /** Flattened session id of the attached page target. */
  declare sessionId: any;
  /** Random per-session attribute the analyzer tags elements with. */
  declare tagAttr: string;
  /** Id of the attached page target. */
  declare targetId: any;
  /** The UA derived from the real browser version and the persona's platform. */
  declare userAgent: any;
  /** Execution context id of the isolated world the analyzer runs in. */
  declare worldContext: any;
  /** Random name of that isolated world. */
  declare worldName: string;
  /** The browser's CDP WebSocket URL. */
  declare wsUrl: any;
  /** Client type reported alongside the Oya client's. */
  clientType = 'cdp';
  /** Actions this client accepts. */
  capabilities = CDP_CAPABILITIES;

  constructor({ wsUrl, provider = 'cdp', onClose, fingerprint, login, onStorage }: any = {}) {
    Object.assign(this, { wsUrl, provider, onClose });
    // The persona carries no UA of its own, it is derived from the real
    // browser's version and this persona's platform in attach(), once the
    // connection can report that version. Reading fingerprint.userAgent here
    // left this null for every persona, so the override never fired and CDP
    // browsers kept HeadlessChrome in the UA and in the request header.
    this.fingerprint = fingerprint || null;
    this.worldName = 'w' + randomBytes(WORLD_NAME_BYTES).toString('hex');
    this.acceptLanguage = fingerprint?.navigator?.languages?.join(',') || null;
    Object.assign(this, { recording: false, recorded: [], recordedSecrets: new Set(), recordedIds: new Set() });
    this.login = login;
    this.loginState = login ? new LoginState(login.origins || {}, onStorage) : null;
  }
}
