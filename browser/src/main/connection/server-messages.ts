/**
 * Message type → handler: everything the server can say to this browser. The
 * socket looks the type up here; an unknown type is ignored.
 */
import { captureProfile } from './profile-capture.ts';
import { HOME_URL } from '../tabs/constants.ts';
import type { AppServices } from '../app/services.ts';
import type { CommandParams } from '../actions/types.ts';
import type { FingerprintProfile } from '../app/persona.ts';
import type { ControlResult, ControlValue } from '../control/control-state.ts';
import type { MirrorOk } from '../mirror/mirror.ts';
import { DEFAULT_STREAM_FPS } from './constants.ts';

/** What the server's messages reach: the session's services. */
export type ServerMessageDeps = Pick<
  AppServices,
  | 'socket'
  | 'persona'
  | 'shell'
  | 'tabs'
  | 'cookies'
  | 'mirror'
  | 'routines'
  | 'control'
  | 'config'
  | 'relay'
  | 'stream'
  | 'commands'
  | 'governance'
>;

/** The persona the server signed this browser in as. */
interface PersonaName {
  /** Its name, shown as the profile. */
  name?: string;
}

/** One message from the server, as parsed JSON: its type and whichever fields that type carries. */
export interface ServerMessage {
  /** Which handler runs it. */
  type: string;
  /** auth_ok: the id the server knows this browser by. */
  browser_id?: string;
  /** auth_ok: the persona's device. */
  fingerprint?: FingerprintProfile;
  /** auth_ok and cookie_sync: cookies to apply. */
  cookies?: unknown[];
  /** The server's clock, in epoch milliseconds. */
  now?: number;
  /** auth_ok: the control state the session starts with. */
  control?: ControlValue;
  /** auth_ok: the persona this browser runs as. */
  persona?: PersonaName;
  /** control_mode and desktop_control_result: the full control state. */
  state?: ControlValue;
  /** control_mode: the mode, when no state is sent. */
  mode?: string;
  /** profile_saved: why the save failed. */
  error?: string;
  /** profile_saved: the sites the server now keeps. */
  sites?: unknown[];
  /** agent-event: the chat run it belongs to. */
  runId?: string;
  /** agent-event: what the agent did. */
  event?: unknown;
  /** mirror_ok: one persona id per captured profile. */
  personaIds?: string[];
  /** mirror_ok: the persona the desktop switches to. */
  defaultPersonaId?: string;
  /** cookie_sync: the pull this answers. */
  pullId?: string;
  /** stream_start: frames per second. */
  fps?: number;
  /** cdp_*: the relayed session. */
  sid?: string;
  /** cdp: one CDP frame. */
  data?: unknown;
  /** cmd: the command's id. */
  id?: string;
  /** cmd: what to do; checked to be a string before it is looked up. */
  action?: unknown;
  /** cmd: the command's arguments. */
  params?: CommandParams;
}

/** Runs one message type. */
type Handler = (router: ServerMessages, msg: ServerMessage) => unknown;

/** The handler for each message type. */
const SERVER_MESSAGES: Record<string, Handler> = {
  control_mode: ({ deps }, msg) => {
    if (msg.state) deps.control.receive(msg.state);
    deps.governance.setMode(msg.state ? deps.control.snapshot().mode : msg.mode);
  },
  desktop_control_result: ({ deps }, msg) => {
    // A desktop_control_result always carries the id of the request it answers.
    deps.control.result(msg as ControlResult);
    if (msg.state) deps.governance.setMode(deps.control.snapshot().mode);
  },
  auth_ok: (router, msg) => router.acceptAuth(msg),
  profile_capture: ({ deps }, msg) => captureProfile(deps, msg.id),
  // A save the person asked for: how many sites the server now keeps, remembered for the profile dialog.
  profile_saved: ({ deps }, msg) => {
    if (!msg.error) deps.config.merge({ lastSync: { at: Date.now(), sites: msg.sites?.length || 0 } });
    deps.config.save();
    deps.shell.send('profile-saved', msg);
  },
  // The project's settings changed elsewhere (the console, the CLI): the model card re-reads them.
  settings_changed: ({ deps }, msg) => deps.shell.send('settings-changed', msg),
  // What the agent is doing in a chat this browser started (its plan, each step, how it ended), for the panel to show live.
  'agent-event': ({ deps }, msg) => deps.shell.send('agent-event', { runId: msg.runId, event: msg.event }),
  // The project's routines changed (another desktop, a run starting or ending): re-read them.
  routines_changed: ({ deps }) => void deps.routines.refresh(),
  cookie_sync: async ({ deps }, msg) => {
    await deps.cookies.applyCookieSync(msg.cookies, { now: msg.now, pullId: msg.pullId });
    deps.cookies.answerPull(msg.pullId);
  },
  // A mirror_ok always names the persona the desktop switches to.
  mirror_ok: ({ deps }, msg) => deps.mirror.onOk(msg as MirrorOk),
  mirror_failed: ({ deps }, msg) => deps.mirror.onFailed(msg),
  ping: ({ deps }) => {
    deps.socket.heard();
    deps.socket.send({ type: 'pong' });
  },
  pong: ({ deps }) => deps.socket.heard(),
  stream_start: ({ deps }, msg) => deps.stream.startStream(msg.fps || DEFAULT_STREAM_FPS),
  stream_stop: ({ deps }) => deps.stream.stopStream(),
  // Commands run concurrently; the queue does not wait for one to finish.
  cmd: ({ deps }, msg) => void deps.commands.handleCommand(msg),
  cdp_open: ({ deps }, msg) => void deps.relay.openCdpRelay(msg.sid),
  cdp: ({ deps }, msg) => deps.relay.cdpRelays.get(msg.sid)?.send(String(msg.data)),
  cdp_close: (router, msg) => router.closeRelay(msg),
};

/** Routes the server's messages to the services they are for. */
export class ServerMessages {
  /** The services the handlers act on; read by the message map. */
  readonly deps: ServerMessageDeps;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: ServerMessageDeps) {
    this.deps = deps;
  }

  /** Runs the handler for `msg.type`, if there is one. */
  handle(msg: ServerMessage): unknown {
    if (!Object.hasOwn(SERVER_MESSAGES, msg.type)) return undefined;
    return SERVER_MESSAGES[msg.type](this, msg);
  }

  /** The server accepted us: take the persona it sent, go online, and share our cookies. */
  async acceptAuth(msg: ServerMessage): Promise<void> {
    const { persona, shell, tabs } = this.deps;
    this.beginAuth(msg);
    await persona.ensureLoginState(msg);
    // Apply fingerprint from the server, the server is the single source of truth.
    // Same API key = same fingerprint on every browser, guaranteed.
    if (msg.fingerprint) await persona.applyServerFingerprint(msg.fingerprint, msg.cookies || [], msg.now);
    this.goOnline(msg);
    if (!shell.browsingMode) tabs.enterBrowsingMode(this.deps.governance.configuration ? 'about:blank' : HOME_URL);
    await this.shareProfile();
  }

  /** Suspend publication before any new identity's asynchronous initialization starts. */
  private beginAuth(msg: ServerMessage): void {
    const socket = this.deps.socket;
    socket.ready = false;
    socket.reconnectAttempts = 0;
    if (msg.browser_id) socket.browserId = msg.browser_id;
  }

  /** Sends our cookies to the pool, then, on first sign-in only, mirrors the user's real browser. */
  private async shareProfile(): Promise<void> {
    // Changes made while the socket was down go first: a dump cannot say that a cookie was deleted.
    this.deps.cookies.flushCookieChanges();
    if ((await this.deps.cookies.dumpCookies()) === false) throw Error('Profile cookies could not be sent');
    if (!(await this.deps.persona.flushStorage())) throw Error('Profile storage could not be sent');
    this.deps.socket.send({ type: 'profile_flush' });
    // A no-op once done; it reconnects as the mirrored persona itself.
    void this.deps.mirror.maybeRun();
    // The project's routines, read fresh on every sign-in (and handed over from config.json once).
    void this.deps.routines.refresh();
  }

  /** Marks the socket ready, takes the control state, and starts the heartbeat. */
  private goOnline(msg: ServerMessage): void {
    const { socket, control, config, governance } = this.deps;
    socket.ready = true;
    control.connect(msg.control);
    if (msg.control) governance.setMode(msg.control.mode);
    config.values.profileName = msg.persona?.name || 'Default';
    config.save();
    socket.startPingLoop();
    socket.sendStatus();
  }

  /** A relayed CDP session closed on the server's side. */
  closeRelay(msg: ServerMessage): void {
    const sock = this.deps.relay.cdpRelays.get(msg.sid);
    this.deps.relay.cdpRelays.delete(msg.sid);
    sock?.close();
  }
}
