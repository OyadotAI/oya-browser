/** Device-specific macOS recovery explains only observed failures and never grants or retries capture. */
import type { WebContents, MessageBoxOptions, MessageBoxReturnValue } from 'electron';
import type { AppServices } from './services.ts';
import { mediaDeniedPrompt, type MediaDenial } from './media-prompts.ts';
import { SCREEN_RECORDING_SETTINGS_URL } from '../screen-share/index.ts';
import { MEDIA_MANUAL_ACTION, MEDIA_SETTINGS_OPEN_MS } from './constants.ts';
import { withinTime } from '../../shared/within-time.ts';

/** Fixed destinations cannot be replaced by requesting web content. */
const SETTINGS = {
  audio: 'x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone',
  video: 'x-apple.systempreferences:com.apple.preference.security?Privacy_Camera',
  screen: SCREEN_RECORDING_SETTINGS_URL,
};
/** A recovery belongs to an already-admitted document and window. */
interface RecoveryRequest {
  /** Exact native requester. */ contents: WebContents;
  /** Original committed/request generation. */ generation: number;
  /** Rechecks window, document, session, profile, origin and control. */ current(): boolean;
  /** Failed device and actual OS status. */ failure: MediaDenial;
}
/** Presentation is injected so dialogs always use the requester's own window. */
type Ask = (contents: WebContents, options: MessageBoxOptions) => Promise<MessageBoxReturnValue>;
/** Independent device explanations avoid suppressing camera recovery after a microphone denial. */
export class MediaRecovery {
  /** OS launch is the only external effect; it never changes permissions itself. */
  private readonly electron: Pick<AppServices['electron'], 'shell'>;
  /** One live native dialog at a time remains owned by the shared media queue. */
  private readonly ask: Ask;
  /** Each device may be explained once for the current document. */
  private readonly seen = new WeakMap<WebContents, Map<MediaDenial['device'], number>>();
  /** Native launch and requester-bound presentation are composed by the media gate. */
  constructor(electron: Pick<AppServices['electron'], 'shell'>, ask: Ask) {
    this.electron = electron;
    this.ask = ask;
  }
  /** Settings is explicit recovery, never evidence that access was granted. */
  async show(request: RecoveryRequest): Promise<false> {
    if (!request.current() || this.explained(request)) return false;
    const options = mediaDeniedPrompt(request.failure);
    const answer = await this.ask(request.contents, options);
    if (request.failure.status === 'denied' && answer.response === options.defaultId && request.current())
      await this.openSettings(request, options);
    return false;
  }
  /** Document identity and device both participate in suppression. */
  private explained({ contents, generation, failure }: RecoveryRequest): boolean {
    const devices = this.seen.get(contents) || new Map();
    if (devices.get(failure.device) === generation) return true;
    devices.set(failure.device, generation);
    this.seen.set(contents, devices);
    return false;
  }
  /** A failed OS opener retains a readable manual path without starting another permission loop. */
  private async openSettings(request: RecoveryRequest, options: MessageBoxOptions): Promise<void> {
    try {
      await withinTime(this.electron.shell.openExternal(SETTINGS[request.failure.device]), MEDIA_SETTINGS_OPEN_MS);
    } catch {
      if (request.current()) await this.ask(request.contents, manualPrompt(options));
    }
  }
}
/** Manual instructions remain available when the platform refuses the deep link. */
function manualPrompt(options: MessageBoxOptions): MessageBoxOptions {
  return {
    ...options,
    message: 'Open System Settings manually',
    detail: `${options.detail} Oya couldn’t open System Settings. Open it manually using the path above.`,
    ...MEDIA_MANUAL_ACTION,
  };
}
