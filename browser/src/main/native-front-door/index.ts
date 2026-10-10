/** External CDP compatibility that terminates in browser-owned native capabilities only. */
export { startNativeFrontDoor } from './server.ts';
export { nativeDoorConfig } from './constants.ts';
export type { NativeBackend, NativeTarget, NativeDoorOptions, NativeSubscription } from './types.ts';
export { NativeRelayConnection, NativeRelayQueue } from './relay.ts';
