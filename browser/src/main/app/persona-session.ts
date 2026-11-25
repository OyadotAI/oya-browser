/** Immutable native session setup with identity fences across all asynchronous work. */
import { canonicalJson } from '../../shared/canonical-json.ts';
import type { Session } from 'electron';
import type { AppServices } from './services.ts';
import type { Persona } from './persona.ts';
import { configureSession, type SessionExtras } from '../identity/session.ts';
import { ExitZone } from '../identity/exit-zone.ts';
import { exitProxy } from '../tabs/protection.ts';
import { capturePersonaPolicy } from './persona-policy.ts';
/** Only the services used to install native partition policy. */
type Deps = Pick<
  AppServices,
  | 'electron'
  | 'nativeBrowsing'
  | 'observer'
  | 'governance'
  | 'mediaPermissions'
  | 'notificationPermissions'
  | 'screenShare'
  | 'externalApps'
  | 'protection'
  | 'passkeys'
  | 'workers'
  | 'downloads'
>;
/** The exact identity and resolved egress of an already configured partition. */
interface NativeSessionPlan {
  /** Immutable serialized persona and governance configuration. */
  key: string;
  /** This partition's resolved exit timezone, restored when switching back. */
  exitZone: string | null;
}

/** The dotted names of the fields two plans disagree on, values left out (they hold a device's fingerprint). */
function changedFields(before: string, after: string, at = ''): string[] {
  const [a, b] = [JSON.parse(before), JSON.parse(after)];
  const keys = [...new Set([...Object.keys(a ?? {}), ...Object.keys(b ?? {})])];
  return keys.flatMap((k) => {
    const [x, y] = [JSON.stringify(a?.[k]), JSON.stringify(b?.[k])];
    if (x === y) return [];
    const nested = a?.[k] && b?.[k] && typeof a[k] === 'object' && typeof b[k] === 'object';
    return nested ? changedFields(x, y, `${at}${k}.`) : [`${at}${k}`];
  });
}

/** A live session's persona or egress really changed: say which fields (never their values), then refuse. */
function refuseChange(before: string, after: string): never {
  console.warn(`[oya] persona plan changed in: ${changedFields(before, after).join(', ') || 'unknown'}`);
  throw Error('Native persona or egress changed; restart Oya to apply it safely');
}

/** One exclusive setup owner for every persistent persona partition. */
export class PersonaSessionSetup {
  /** Active identity owner. */
  private readonly persona: Persona;
  /** Native application dependencies. */
  private readonly deps: Deps;
  /** Exit lookup follows the pinned partition's proxy. */
  private readonly zone: ExitZone;
  /** Completed immutable identity and routing plans. */
  private readonly sessionPlans = new WeakMap<Session, NativeSessionPlan>();
  /** Cold configuration is exclusive; failures remain fenced until process restart. */
  private readonly configuring = new WeakSet<Session>();
  /** Capture dependencies without creating any renderer. */
  constructor(persona: Persona, deps: Deps) {
    this.persona = persona;
    this.deps = deps;
    this.zone = new ExitZone({ net: deps.electron.net });
  }
  /** A denied external protocol is offered to the human-confirmed handoff. */
  private readonly externalApp: SessionExtras['externalApp'] = (url, contents) =>
    void this.deps.externalApps.request(url, contents);
  /** Keep protocol prompting injected through the persona session boundary. */
  private sessionExtras(): SessionExtras {
    const { nativeBrowsing, observer, governance, mediaPermissions: media, screenShare } = this.deps;
    const notifications = this.deps.notificationPermissions;
    return { nativeBrowsing, observer, governance, media, screenShare, notifications, externalApp: this.externalApp };
  }

  /** Configure the persistent browser session, user-agent, cookies, privacy. */
  async configure(): Promise<void> {
    const session = this.persona.session();
    const plan = canonicalJson({ profile: this.persona.active, governance: this.deps.governance.configuration });
    if (this.reuseSessionPlan(session, plan)) return;
    if (this.configuring.has(session))
      throw Error('Native session configuration already in progress or incomplete; restart Oya');
    this.configuring.add(session);
    return this.install(session, plan);
  }

  /** Publish a plan only after its complete guarded configuration succeeds. */
  private async install(session: Session, plan: string): Promise<void> {
    const assertCurrent = capturePersonaPolicy(this.persona, this.deps.governance);
    await this.configureColdSession(session);
    assertCurrent();
    this.sessionPlans.set(session, { key: plan, exitZone: this.persona.exitZone });
    this.configuring.delete(session);
  }

  /** Refuse identity or egress changes before changing a live session's proxy or other resources. */
  private reuseSessionPlan(session: Session, plan: string): boolean {
    const existing = this.sessionPlans.get(session);
    if (existing === undefined) return false;
    if (existing.key !== plan) refuseChange(existing.key, plan);
    this.deps.protection.assertSession(session);
    this.persona.exitZone = existing.exitZone;
    return true;
  }

  /** Resolve egress before the cold native identity is locked by the first renderer. */
  private async configureColdSession(session: Session): Promise<void> {
    const { governance } = this.deps;
    const assertCurrent = capturePersonaPolicy(this.persona, governance);
    const profile = this.persona.active;
    this.deps.passkeys?.install(session);
    // A governed browser keeps Electron's own download handling; the person's downloads list is for the desktop.
    if (!governance.configuration) this.deps.downloads?.install(session, this.persona.partitionName());
    await configureSession(this.deps.electron.app, session, profile, this.sessionExtras());
    assertCurrent();
    // Only a proxied persona asks: without one the zone is this machine's own, known already.
    await this.protectSession(session, assertCurrent);
  }

  /** Resolve the pinned proxy before synchronously installing page and worker identity. */
  private async protectSession(session: Session, assertCurrent: () => void): Promise<void> {
    const proxy = exitProxy(this.persona.active, this.deps.governance.configuration?.proxy);
    const exitZone = proxy ? await this.zone.timezone(session, proxy) : null;
    assertCurrent();
    this.persona.exitZone = exitZone;
    this.deps.protection.configureSession(session);
    await this.deps.workers?.cover();
    assertCurrent();
  }
}
