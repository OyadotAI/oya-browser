/** WKWebView's isolated document-start adapter exposes only document registration to native main. */
import { DebugDocument } from './debug-runtime.ts';
/** WebKit supplies this handler only in the browser-owned isolated content world. */
interface RegistrationWindow {
  /** Fixed WebKit bridge. */ webkit: {
    /** Fixed handlers. */ messageHandlers: {
      /** Registration-only handler. */ keel_debug: {
        /** Send bounded identity metadata. */ postMessage(value: string): void;
      };
    };
  };
}
const runtime = new DebugDocument(
  () => {
    throw Error('Page-world eval must use the native WK adapter');
  },
  (event) =>
    (window as unknown as RegistrationWindow).webkit.messageHandlers.keel_debug.postMessage(JSON.stringify(event)),
);
Object.defineProperty(globalThis, '__keelDebug', { value: runtime, configurable: false, writable: false });
runtime.register();
