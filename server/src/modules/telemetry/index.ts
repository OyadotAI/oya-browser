/**
 * Telemetry: product events and ops messages. Other modules import `track`
 * from here and call the one function for what just happened.
 */
export { track, clientOf } from './service.ts';
export { trackDownloads } from './downloads.ts';
export { versionOf, UNKNOWN as UNKNOWN_VERSION } from './version.ts';
export type { EventName, EventProps, SignupMethod, Who } from './catalog.ts';
