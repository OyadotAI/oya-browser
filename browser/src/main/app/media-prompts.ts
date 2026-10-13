/**
 * The words the device dialogs say: microphone and camera, screen share, and
 * the macOS denial. Kept apart from the rules so the copy reads as one family
 * and a test can pin it.
 */
import type { MessageBoxOptions } from 'electron';
import { END_CALL_NAMED_SITES } from './constants.ts';

/** What a site can ask for in the Allow dialog. */
export type DeviceKind = 'audio' | 'video';

/** The line every Allow dialog ends on: what Allow means, and where to undo it. */
const REMEMBERED = 'Allow is remembered for this site in this profile; forget it any time under History and bookmarks.';

/** Block first, so Enter and Escape both refuse. */
const BLOCK_OR_ALLOW: Pick<MessageBoxOptions, 'buttons' | 'defaultId' | 'cancelId'> = {
  buttons: ['Block', 'Allow'],
  defaultId: 0,
  cancelId: 0,
};

/** Tells the person exactly which devices the website requested. */
export function allowDevicesPrompt(origin: string, types: DeviceKind[]): MessageBoxOptions {
  const devices = types.map((type) => (type === 'audio' ? 'microphone' : 'camera')).join(' and ');
  return {
    message: `Allow ${devices}?`,
    detail: `${origin} wants to use your ${devices}. Allow only if you trust this site. ${REMEMBERED}`,
    ...BLOCK_OR_ALLOW,
  };
}

/** Site consent permits requesting a share; the person still chooses a source for every stream. */
export function shareScreenPrompt(origin: string): MessageBoxOptions {
  return {
    message: `Share your screen with ${origin}?`,
    detail: `You will choose an Oya tab, a window or an entire screen before sharing starts. You choose again for every new share. ${REMEMBERED}`,
    ...BLOCK_OR_ALLOW,
  };
}

/** Explains an OS denial rather than leaving a call silently without a device. */
export const OS_DENIED_PROMPT: MessageBoxOptions = {
  message: 'Device access is blocked in macOS',
  detail:
    'Allow Oya Browser in System Settings → Privacy & Security → Microphone, Camera or Screen & System Audio Recording, then rejoin the call.',
  buttons: ['OK'],
};

/** Screen Recording has its own pane and may require adding the installed application manually. */
export const SCREEN_DENIED_PROMPT: MessageBoxOptions = {
  message: 'Allow Oya to share your screen',
  detail:
    'In System Settings → Privacy & Security → Screen & System Audio Recording, enable Oya Browser. If it is missing, click + and select Oya Browser in Applications. Then quit and reopen Oya and try sharing again.',
  buttons: ['Not now', 'Open System Settings'],
  defaultId: 1,
  cancelId: 0,
};

/** What would end the call: closing one window, or quitting the app. */
export type EndCallKind = 'close' | 'quit';

/** The end-call dialog's words for one way out. */
interface EndCallCopy {
  /** The question, in bold. */
  message: string;
  /** What ends the call, opening the last sentence of the detail. */
  ends: string;
  /** The button that goes ahead. */
  button: string;
}

/** The end-call dialog's words for each way out. */
const END_CALL_COPY: Record<EndCallKind, EndCallCopy> = {
  close: { message: 'Close this window and end your call?', ends: 'Closing this window', button: 'Close Window' },
  quit: { message: 'Quit Oya Browser and end your call?', ends: 'Quitting', button: 'Quit' },
};

/** Sites, not tabs: one, two, or the first and how many others. Hostnames only, since meeting links carry passwords. */
export function sitesLabel(hosts: string[]): string {
  if (hosts.length <= END_CALL_NAMED_SITES) return hosts.join(' and ');
  const others = hosts.length - 1;
  return `${hosts[0]} and ${others} other sites`;
}

/** Cancel first, so Return and Escape both keep the call. */
export function endCallPrompt(kind: EndCallKind, hosts: string[]): MessageBoxOptions {
  const copy = END_CALL_COPY[kind];
  return {
    type: 'warning',
    message: copy.message,
    detail: `${sitesLabel(hosts)} ${hosts.length > 1 ? 'are' : 'is'} using your camera, microphone or screen. ${copy.ends} ends the call.`,
    buttons: ['Cancel', copy.button],
    ...{ defaultId: 0, cancelId: 0, noLink: true },
  };
}
