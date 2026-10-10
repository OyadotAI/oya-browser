/** Compose ephemeral contexts with the same native session policy as ordinary tabs. */
import type { NativeDownloads } from '../native-downloads/index.ts';
import type { Session } from 'electron';
import type { AppServices } from './services.ts';
import { NativeContexts } from '../native-contexts/index.ts';
import { capturePersonaPolicy } from './persona-policy.ts';
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
    const assertCurrent = capturePersonaPolicy(d.persona, d.governance);
    await this.routeSession(session);
    assertCurrent();
    d.protection.configureSession(session);
    this.installResources(session);
  }
  /** Install permissions and proxy before any private renderer exists. */
  private routeSession(session: Session): Promise<void> {
    const d = this.deps;
    return configureSession(d.electron.app, session, d.persona.active, {
      nativeBrowsing: true,
      governance: d.governance,
    });
  }
  /** Resource activation follows complete native identity installation. */
  private installResources(session: Session): void {
    this.resources.downloads.install(session);
    this.resources.network.install(session);
  }
  /** Cancel every private resource even if one cancellation fails; errors never imply successful disposal. */
  close(session: Session): void {
    retirePrivateResources([
      () => this.resources.network.remove(session),
      () => this.resources.downloads.remove(session),
      () => this.closeOwnedTabs(session),
    ]);
  }
  /** A failure closing one exact-session tab must not strand its siblings or touch another session. */
  private closeOwnedTabs(session: Session): void {
    const tabs = (this.deps.windows?.allTabs() || []).filter((tab) => tab.view.webContents.session === session);
    retirePrivateResources(
      tabs.map((tab) => () => this.deps.windows?.owner(tab.id)?.tabs.closeTab(tab.id, { keepOne: false })),
    );
  }
}
/** Attempt a synchronous resource without preventing later native cleanup steps. */
function attemptPrivateResource(step: () => void, failures: unknown[]): void {
  try {
    step();
  } catch (error) {
    failures.push(error);
  }
}
/** Preserve every cancellation failure after all independent resources have been attempted. */
function retirePrivateResources(steps: Array<() => void>): void {
  const failures: unknown[] = [];
  for (const step of steps) attemptPrivateResource(step, failures);
  if (failures.length) throw new AggregateError(failures, 'Private context resources could not all be retired');
}
