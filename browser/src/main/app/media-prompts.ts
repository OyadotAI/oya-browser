/**
 * The words the device dialogs say: microphone and camera, screen share, and
 * the macOS denial. Kept apart from the rules so the copy reads as one family
 * and a test can pin it.
 */
import type { MessageBoxOptions, SystemPreferences } from 'electron';
import { END_CALL_NAMED_SITES, MEDIA_SETTINGS_ACTION } from './constants.ts';

/** What a site can ask for in the Allow dialog. */
export type DeviceKind = 'audio' | 'video';
/** OS denial applies to one specific device, never an inferred bundle of permissions. */
export interface MediaDenial {
  /** The device whose native request failed. */ device: DeviceKind | 'screen';
  /** Observed status distinguishes denial, restriction and an unknown failure. */ status: ReturnType<
    SystemPreferences['getMediaAccessStatus']
  >;
}

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
export function mediaDeniedPrompt(failure: MediaDenial): MessageBoxOptions {
  const device = failure.device === 'audio' ? 'Microphone' : failure.device === 'video' ? 'Camera' : 'Screen recording';
  if (failure.status === 'restricted') return restrictedPrompt(device);
  if (failure.status !== 'denied') return unavailablePrompt(device);
  return failure.device === 'screen' ? SCREEN_DENIED_PROMPT : blockedDevicePrompt(device);
}
/** A confirmed device denial links only to that device's permission pane. */
function blockedDevicePrompt(device: string): MessageBoxOptions {
  return {
    message: `${device} access is blocked by macOS.`,
    detail: `Allow Oya Browser in System Settings → Privacy & Security → ${device}, then return and try again.`,
    buttons: ['Not now', `Open ${device} Settings`],
    ...MEDIA_SETTINGS_ACTION,
  };
}
/** Organization restrictions cannot be fixed by promising an enable-toggle action. */
function restrictedPrompt(device: string): MessageBoxOptions {
  return {
    message: `Your Mac restricts ${device.toLowerCase()} access.`,
    detail: 'This setting may be managed by your organization.',
    buttons: ['OK'],
  };
}
/** Failed native requests without a confirmed denial retain honest, generic recovery. */
function unavailablePrompt(device: string): MessageBoxOptions {
  return {
    message: `Couldn’t request ${device.toLowerCase()} access.`,
    detail: 'Try again from your meeting. If this continues, check that your device is available.',
    buttons: ['OK'],
  };
}

/** Screen Recording has its own pane and may require adding the installed application manually. */
export const SCREEN_DENIED_PROMPT: MessageBoxOptions = {
  message: 'Allow screen sharing in macOS.',
  detail:
    'In System Settings → Privacy & Security → Screen & System Audio Recording, enable Oya Browser. If it is missing, click + and select Oya Browser in Applications. Oya tab, window and screen sharing are unavailable until access is allowed. If macOS requires reopening Oya, doing so ends active calls; then try sharing again.',
  buttons: ['Not now', 'Open System Settings'],
  ...MEDIA_SETTINGS_ACTION,
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
