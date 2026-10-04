/**
 * Managed-only browser hooks: a governed browser (OYA_GOVERNANCE, set by the
 * managed runtime) refuses requests outside its policies' hosts and every
 * permission prompt. Network isolation remains the outer enforcement boundary;
 * the server's egress proxy applies the same host rules (host-rules.ts).
 */
import type { Session } from 'electron';
import type { ProxyConfig } from '../../anonymity/proxy.ts';
import { matchesHost } from './host-rules.ts';
import { BLANK_PAGE, EGRESS_PROTOCOLS } from './constants.ts';

/** One governance policy's host rules, as the server validated them. */
export interface HostPolicy {
  /** Hosts the browser may reach at all; absent means any. */
  allowedHosts?: string[];
  /** Hosts only a person (human control) may reach; absent means none are held back. */
  humanHosts?: string[];
}

/** What the managed runtime hands the browser in OYA_GOVERNANCE. */
export interface GovernanceConfiguration {
  /** Every policy must allow a request. */
  policies: HostPolicy[];
  /** The egress proxy every request goes through, which wins over the persona's. */
  proxy?: ProxyConfig | null;
}

/** The parts of an Electron session governance hooks into. */
export type GovernedSession = Pick<Session, 'webRequest' | 'setPermissionRequestHandler' | 'setPermissionCheckHandler'>;

/** OYA_GOVERNANCE's JSON, or null for an ungoverned browser. Malformed JSON throws: a governed browser must not start open. */
export function readGovernance(raw: string | undefined): GovernanceConfiguration | null {
  return raw ? (JSON.parse(raw) as GovernanceConfiguration) : null;
}

/** The browser's governance: its policies, and who drives now (humanHosts open only to a person). */
export class Governance {
  /** The policies and proxy, or null when the browser is not governed (then nothing is refused). */
  readonly configuration: GovernanceConfiguration | null;
  /** 'human' while a person holds control, 'agent' otherwise. */
  private mode: 'agent' | 'human' = 'agent';

  /** `configuration` is readGovernance's result. */
  constructor(configuration: GovernanceConfiguration | null) {
    this.configuration = configuration;
  }

  /** Takes the control mode the server reports; anything but 'human' counts as the agent. */
  setMode(value: unknown): void {
    this.mode = value === 'human' ? 'human' : 'agent';
  }

  /** Whether a request to `raw` may go out. */
  allowed(raw: string): boolean {
    if (!this.configuration || raw === BLANK_PAGE) return true;
    const url = URL.parse(raw);
    if (!url || !EGRESS_PROTOCOLS.includes(url.protocol)) return false;
    return this.configuration.policies.every((policy) => this.permits(policy, url.hostname));
  }

  /** Refuses the session's requests outside the policies, and every permission. A no-op when ungoverned. */
  install(session: GovernedSession): void {
    if (!this.configuration) return;
    session.webRequest.onBeforeRequest((details, callback) => callback({ cancel: !this.allowed(details.url) }));
    session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
    session.setPermissionCheckHandler(() => false);
  }

  /** Whether one policy lets `host` through in the current mode. */
  private permits(policy: HostPolicy, host: string): boolean {
    if (policy.allowedHosts && !matchesHost(host, policy.allowedHosts)) return false;
    return !policy.humanHosts || !matchesHost(host, policy.humanHosts) || this.mode === 'human';
  }
}
