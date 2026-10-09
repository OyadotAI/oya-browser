/** Compose ephemeral contexts with the same native session policy as ordinary tabs. */
import type { NativeDownloads } from '../native-downloads/index.ts';
import type { Session } from 'electron';
import type { AppServices } from './services.ts';
import { NativeContexts } from '../native-contexts/index.ts';
import { configureSession } from '../identity/session.ts';
/** Exclusively owned session resources are installed after policy and revoked before tab disposal. */
interface Resources {
  /** Native download owner. */
  downloads: NativeDownloads;
  /** Native original-request pipeline. */
  network: {
    /** Install only on a newly configured private partition. */
    install(session: Session): unknown;
    /** Cancel held native requests before closing the renderer. */
    remove(session: Session): void;
  };
}
/** Never create an isolated context through legacy persona instrumentation. */
export function appNativeContexts(deps: AppServices, resources: Resources): NativeContexts {
  const setup = new ContextSetup(deps, resources);
  return new NativeContexts({
    create: (partition) => deps.electron.session.fromPartition(partition, { cache: false }),
    configure: (session) => setup.configure(session),
    close: (session) => setup.close(session),
  });
}
/** Native session policy precedes every private renderer, preserving existing refusal semantics. */
class ContextSetup {
  /** Application policy services. */
  private readonly deps: AppServices;
  /** Context-specific network and file resources. */
  private readonly resources: Resources;
  /** Keep policy and resources separate from protocol input. */
  constructor(deps: AppServices, resources: Resources) {
    this.deps = deps;
    this.resources = resources;
  }
  /** No copied cookie jar or silently bypassed persona/proxy policy. */
  async configure(session: Session): Promise<void> {
    const d = this.deps;
    if (!d.nativeBrowsing) throw Error('Browser contexts require native browsing mode');
    await configureSession(d.electron.app, session, d.persona.active, {
      nativeBrowsing: true,
      governance: d.governance,
    });
    this.resources.downloads.install(session);
    this.resources.network.install(session);
  }
  /** Cancel private work first, then close all exact-session tabs including adopted popups. */
  close(session: Session): void {
    this.resources.network.remove(session);
    this.resources.downloads.remove(session);
    for (const tab of this.deps.windows?.allTabs() || [])
      if (tab.view.webContents.session === session)
        this.deps.windows?.owner(tab.id)?.tabs.closeTab(tab.id, { keepOne: false });
  }
}
