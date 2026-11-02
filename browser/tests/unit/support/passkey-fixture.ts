/** Native passkey seams with independent owning window, document, profile and human-control state. */
import { EventEmitter } from 'node:events';
import { Passkeys } from '../../../src/main/app/passkeys.ts';
import { WindowTabEvents } from '../../../src/main/windows/tab-events.ts';

/** Minimal native lifetime APIs; no browser, password, biometric or network is involved. */
export function fixture() {
  const contents = Object.assign(new EventEmitter(), { isDestroyed: () => false, session: {} });
  const frame: any = { detached: false, isDestroyed: () => false, parent: null };
  const window = Object.assign(new EventEmitter(), { isDestroyed: () => false, isFocused: () => true });
  const tab: any = { id: 2, view: { webContents: contents } };
  const owner: any = {
    tabs: { activeTabId: tab.id, list: [tab], find: (id) => owner.tabs.list.find((item) => item.id === id) },
    shell: { window },
  };
  const details = {
    frame,
    relyingPartyId: 'example.com',
    accounts: [
      { credentialId: 'one', name: 'Person' },
      { credentialId: 'two', name: 'Other' },
    ],
  };
  const boxes: any[] = [],
    parents: any[] = [];
  const deps: any = {
    governance: { configuration: null },
    agentTab: { humanMay: () => true },
    persona: { policyEpoch: Symbol(), partitionName: () => 'persist:one' },
    windows: {
      owner: (id) => (owner.tabs.find(id) ? owner : undefined),
      ownerOfContents: (page) => (page === contents ? owner : undefined),
      tabEvents: new WindowTabEvents(),
    },
    electron: {
      app: new EventEmitter(),
      webContents: { fromFrame: () => contents },
      dialog: {
        showMessageBox: async (parent: any, options: any) => {
          parents.push(parent);
          boxes.push(options);
          return { response: 2 };
        },
      },
    },
  };
  return { contents, details, boxes, parents, owner, tab, window, deps, service: new Passkeys(deps) };
}
