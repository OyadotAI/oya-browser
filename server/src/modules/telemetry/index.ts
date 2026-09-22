/**
 * Telemetry: product events and ops messages. Other modules import `track`
 * from here and call the one function for what just happened.
 */
export { track, clientOf } from './service.ts';
export type { EventName, EventProps, Who } from './catalog.ts';
