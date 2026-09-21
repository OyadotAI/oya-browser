/**
 * The install wizard's shapes: the answers it saves to oya-install.json, the
 * secrets it never saves, and the preflight checks it prints.
 */

/** Everything an install decides. Saved (with no secrets) so a rerun reproduces it with --config. */
export interface Answers {
  /** The format version of the saved file. */
  version: 1;
  /** Where the control plane runs. */
  host: string;
  /** What holds sessions, keys and personas. */
  database: string;
  /** What runs the browsers. */
  fleet: string;
  /** Always-on browser workers, for the docker-workers fleet. */
  workers: number;
  /** The Kubernetes fleet's settings, for the k8s fleet. */
  k8sFleet?: {
    /** Namespace for browser pods. */
    namespace: string;
    /** The browser image; resolved to a digest at provision time. */
    image: string;
    /** The control plane's WebSocket URL, as reachable from the cluster. */
    controlUrl: string;
    /** The egress proxy's URL, as reachable from the cluster. */
    proxyUrl: string;
  };
  /** The agent LLM. */
  llm: {
    /** Which vendor, or 'skip'. */
    provider: string;
    /** Its OpenAI-compatible endpoint. */
    baseUrl: string;
    /** The default model. */
    model: string;
  };
  /** Where clients and browsers reach the control plane. */
  publicUrl: string;
  /** Optional services. */
  optional: {
    /** CAPTCHA solver, or empty for none. */
    captcha: string;
    /** Storage bucket for recordings, or empty to keep them on disk. */
    recordingBucket: string;
    /** Whether /metrics gets a scrape token. */
    metrics: boolean;
  };
}

/** Values that must never reach oya-install.json. */
export type Secrets = Record<string, string>;

/** What the interview produced. */
export interface Interview {
  /** The answers to save. */
  answers: Answers;
  /** Credentials, for .env only. */
  secrets: Secrets;
  /** The connection string migrations run against, or empty to skip them. */
  migrateUrl: string;
}

/** One preflight check. */
export interface Check {
  /** Whether it passed. */
  ok: boolean;
  /** What was checked. */
  label: string;
  /** What was found. */
  detail?: string;
  /** Whether failing it stops a real install. */
  fatal?: boolean;
}
