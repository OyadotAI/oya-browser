/**
 * What the admin API answers, as the page reads it.
 */
import type { Comparison, DownloadRow } from './model';

/** One person's usage this month. */
export interface Person {
  /** Their user id. */
  userId: string;
  /** Their email, when known. */
  email: string | null;
  /** Cloud browser time, in seconds. */
  cloud_seconds: number;
  /** Agent steps. */
  agent_steps: number;
}

/** A self-hosted install as its last ping described it. */
export interface Install {
  /** Its random id. */
  install_id: string;
  /** The release it runs. */
  version: string;
  /** Browsers connected. */
  browsers: number;
  /** Most cloud browsers at once since the ping before. */
  peak_cloud: number;
  /** Its license id, or null. */
  license_id: string | null;
  /** Pings received. */
  pings: number;
  /** When it last pinged. */
  last_seen: string;
}

/** Accounts and plans. */
export interface Accounts {
  /** Accounts in all. */
  total: number;
  /** Signups per day. */
  signups: { /** The day. */ day: string; /** How many. */ count: number }[];
  /** Paying people by plan. */
  byPlan: Record<string, number>;
  /** Paying people whose last payment failed. */
  pastDue: number;
}

/** The counters each day carries. */
export type Counted =
  | 'signups'
  | 'active'
  | 'agent_steps'
  | 'cloud_seconds'
  | 'browsers_started'
  | 'commands'
  | 'installers'
  | 'update_checks'
  | 'new_installs'
  | 'revenue_cents';

/** One UTC day's counters. */
export type Day = Record<Counted, number> & {
  /** The day, YYYY-MM-DD. */
  day: string;
};

/** The days shown and how they trend. */
export interface Growth {
  /** One row per day, oldest first, today last. */
  days: Day[];
  /** The last 7 days against the 7 before. */
  week: Record<Counted, Comparison>;
  /** Yesterday against the day before. */
  day: Record<Counted, Comparison>;
  /** Distinct people active today, this week and this month. */
  reach: { /** Today. */ today: number; /** Last 7 days. */ week: number; /** Last 30 days. */ month: number };
}

/** Revenue as Stripe has it. */
export interface Revenue {
  /** Whether Stripe is set up here. */
  enabled: boolean;
  /** Monthly recurring revenue, in cents. */
  mrrCents: number;
  /** Why Stripe could not be read; empty when it could. */
  error: string;
}

/** The overview. */
export interface Overview {
  /** Day by day growth. */
  growth: Growth;
  /** Revenue from Stripe. */
  revenue: Revenue;
  /** Accounts and plans. */
  accounts: Accounts;
  /** Heaviest users by cloud hours and by steps. */
  top: { /** By cloud hours. */ cloud: Person[]; /** By steps. */ steps: Person[] };
  /** Installs, counted and listed. */
  installs: {
    /** In all. */ total: number;
    /** Pinged this week. */ active: number;
    /** Unlicensed past the cap. */ overCap: number;
    /** Most recent first. */ list: Install[];
  };
  /** Download counts per day. */
  downloads: DownloadRow[];
  /** Browsers connected now. */
  fleet: {
    /** In all. */ total: number;
    /** In the cloud. */ cloud: number;
    /** By provider. */ byProvider: Record<string, number>;
  };
}

/** An issued license. */
export interface License {
  /** Its id. */
  id: string;
  /** Who it is for. */
  licensee: string;
  /** Cloud browsers at once. */
  max_concurrent: number;
  /** When it ends. */
  expires_at: string;
  /** Who issued it. */
  created_by: string | null;
  /** When it was revoked, if it was. */
  revoked_at: string | null;
}

/** A key as the lookup shows it. */
export interface KeyShown {
  /** Its first characters. */
  prefix: string;
  /** Its label. */
  label: string;
  /** When it was made. */
  created_at: string;
  /** When it was last used. */
  last_used_at: string | null;
}

/** A person found by email. */
export interface Found {
  /** Their profile row. */
  profile: {
    /** Their user id. */ id: string;
    /** Their email. */ email: string;
  };
  /** Their plan and period. */
  standing: {
    /** The plan. */ plan: string;
    /** Its status. */ status: string | null;
    /** The period's start. */ since: string;
  };
  /** What they used this period. */
  used: Record<string, number>;
  /** Their subscription row, if any. */
  subscription: { /** Their Stripe customer. */ stripe_customer_id?: string } | null;
  /** Complimentary access, independent of Stripe. */
  override?: { /** Plan, or null to follow Stripe. */ plan: string | null } | null;
  /** Support grants for the period shown. */
  grants?: Grant[];
  /** Their keys. */
  keys: KeyShown[];
}

/** One immutable support grant. */
export interface Grant {
  /** Extra agent steps; absent on older servers. */ agent_steps?: number;
  /** Stable request id. */ id: string;
  /** Extra browser time. */ cloud_seconds: number;
  /** Hosted model credit, in micro-USD. */ hosted_llm_microusd: number;
  /** Why support issued it. */ reason: string;
  /** When it was issued. */ created_at: string;
}
