/** A real incognito session over a hermetic fake Electron session, for tests about what incognito tabs never leak. */
import { IncognitoSession } from '../../../src/main/incognito/index.ts';

/** The session methods incognito setup and a tab's teardown touch. */
const fakeSession = () => ({
  setPermissionRequestHandler() {},
  setPermissionCheckHandler() {},
  setProxy: async () => {},
  cookies: { flushStore: async () => {} },
  flushStorageData() {},
});

/** Answers a fresh session marked incognito, protected through the real setup path. */
export function incognitoSession(): Promise<any> {
  const deps: any = {
    nativeBrowsing: true,
    protection: { configureSession() {} },
    persona: { active: null },
    governance: { configuration: null, install() {} },
    externalApps: { request: () => false },
    electron: { app: {}, session: { fromPartition: fakeSession } },
  };
  return new IncognitoSession(deps).get();
}
