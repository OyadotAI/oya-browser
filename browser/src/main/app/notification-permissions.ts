/**
 * Website notifications, with the person's consent. A site asks once per profile from the
 * tab the person is looking at; Allow and Block are both remembered (site-permissions.ts).
 * A remembered Allow then holds in a background tab too, which is the point: mail arrives
 * while another tab is in front. Never for a tab an agent drives, a governed browser or a
 * private window. Electron asks the request handler again before it shows each notification,
 * so a remembered answer must settle without a box. A service worker's notification comes
 * with no tab at all; it is allowed while the person has a tab open on that site.
 */
import type { WebContents } from 'electron';
import type { AppServices } from './services.ts';
import type { Tab } from '../tabs/types.ts';
import { mediaOrigin } from './media-gate.ts';
import { mediaOwner } from './media-owner.ts';
import { notificationsPrompt } from './media-prompts.ts';
import { ALLOW_MEDIA_CHOICE } from './constants.ts';

/** The consent dialog and its rules, the tabs and who holds them, and the remembered answers. */
type Deps = Pick<
  AppServices,
  'mediaPermissions' | 'tabs' | 'shell' | 'windows' | 'persona' | 'agentTab' | 'governance' | 'sitePermissions'
>;

/** What the person decided for a site: allowed, blocked, or not asked yet. */
type Decision = 'allowed' | 'blocked' | null;

/** Notification consent per profile and site; background tabs keep a remembered Allow. */
export class NotificationPermissions {
  /** Services are injected so the rules are hermetic in tests. */
  private readonly deps: Deps;

  /** `deps` are the main-process services (see services.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Whether `contents` (null for a service worker) may show notifications now: a remembered Allow for its secure site. Never prompts. */
  check(contents: WebContents | null, origin: string): boolean {
    const site = contents ? this.site(contents, origin) : this.workerSite(origin);
    return !!site && this.decision(site) === 'allowed';
  }

  /** A remembered answer settles at once; a first ask from the tab in front shows the box; anything else is refused. */
  request(contents: WebContents | null, requestingUrl: string | undefined): Promise<boolean> {
    if (!contents) return Promise.resolve(this.check(null, requestingUrl ?? ''));
    const site = this.site(contents, requestingUrl);
    const decision = site && this.decision(site);
    if (!site || decision) return Promise.resolve(decision === 'allowed');
    return this.firstAsk(contents, site, requestingUrl);
  }

  /** Only the tab in front may raise the box, one at a time; a background tab's first ask is refused. */
  private firstAsk(contents: WebContents, site: string, requestingUrl: string | undefined): Promise<boolean> {
    const { gate } = this.deps.mediaPermissions;
    if (gate.eligible(contents, requestingUrl) !== site) return Promise.resolve(false);
    const current = gate.guard(contents, site);
    return gate.locked(
      current,
      () => this.ask(contents, site, current),
      gate.decisionKey(contents, site, ['notifications']),
    );
  }

  /** Asks, unless a duplicate request already got the answer; stores it only if the same document is still in front. */
  private async ask(contents: WebContents, site: string, current: () => boolean): Promise<boolean> {
    const decided = this.decision(site);
    if (decided) return decided === 'allowed';
    const { gate } = this.deps.mediaPermissions;
    const allowed = await gate.allowedBy(notificationsPrompt(site), ALLOW_MEDIA_CHOICE, contents);
    return current() && this.remember(site, allowed);
  }

  /** Stores the person's answer for `site` in this profile; answers whether it was Allow. */
  private remember(site: string, allowed: boolean): boolean {
    this.deps.sitePermissions.grant(this.partition(), site, [allowed ? 'notifications' : 'notifications-blocked']);
    return allowed;
  }

  /**
   * The secure site `contents` is showing, when `raw` is that same site and the tab is the
   * person's: not destroyed, not governed, not private, not driven by an agent. Unlike the
   * device gate, the tab need not be in front.
   */
  private site(contents: WebContents, raw: string | undefined): string | null {
    if (contents.isDestroyed() || this.deps.governance.configuration || !raw) return null;
    const tab = mediaOwner(this.deps, contents)?.tabs.list.find((t) => t.view.webContents === contents);
    if (!tab || !this.personHolds(tab)) return null;
    const origin = mediaOrigin(raw);
    return origin && origin === mediaOrigin(contents.getURL()) ? origin : null;
  }

  /** Neither the tab nor, for a popup window, the tab that opened it is driven by an agent. */
  private personHolds(tab: Tab): boolean {
    const { agentTab } = this.deps;
    return agentTab.humanMay(tab.id) && !(tab.window && tab.openerId !== undefined && !agentTab.humanMay(tab.openerId));
  }

  /** A service worker's site, while a tab of the person's (not an agent's, not private) is open on it in any window. */
  private workerSite(raw: string): string | null {
    const origin = mediaOrigin(raw);
    const tabs = this.deps.windows?.allTabs() ?? this.deps.tabs.list;
    return origin && tabs.some((tab) => this.site(tab.view.webContents, origin) === origin) ? origin : null;
  }

  /** What this profile decided for `site`; a Block wins over an Allow should both ever be stored. */
  private decision(site: string): Decision {
    const { sitePermissions } = this.deps;
    if (sitePermissions.has(this.partition(), site, 'notifications-blocked')) return 'blocked';
    return sitePermissions.has(this.partition(), site, 'notifications') ? 'allowed' : null;
  }

  /** The profile whose decisions apply right now. */
  private partition(): string {
    return this.deps.persona.partitionName();
  }
}
