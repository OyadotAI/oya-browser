/**
 * Every number the install wizard runs on, by name: version floors, ports,
 * file modes, timings and the ports the deployment it writes listens on.
 */

/** Random bytes in each generated secret (43 characters of base64url). */
export const TOKEN_BYTES = 32;
/** How long to wait for a connection before calling a port free. */
export const PORT_PROBE_MS = 700;
/** Directories walked upward looking for the checkout. */
export const REPO_SEARCH_DEPTH = 8;
/** .env holds the KEK and every provider credential: owner read/write only. */
export const ENV_FILE_MODE = 0o600;
/** The permission bits of a file mode. */
export const PERMISSION_BITS = 0o777;
/** File modes are shown in octal. */
export const OCTAL = 8;
/** The control plane's default port. */
export const DEFAULT_PORT = 3100;
/** The egress proxy's port, as written into .env. */
export const EGRESS_PORT = '3128';
/** DNS, the one thing a governed pod may reach directly. */
export const DNS_PORT = 53;
/** How long to wait for /readyz after `docker compose up`. */
export const READY_TIMEOUT_MS = 180_000;
/** How often /readyz is polled. */
export const READY_POLL_MS = 2000;
/** Characters of a digest shown once it is resolved ("sha256:" plus twelve). */
export const DIGEST_PREVIEW = 19;
/** Questions an interactive run numbers. */
export const INTERVIEW_STEPS = 6;
/** Width of the labels in the printed plan. */
export const PLAN_LABEL_WIDTH = 12;
/** Indentation of the JSON files the wizard writes. */
export const JSON_INDENT = 2;

/** The compose file uses the `env_file: [{path, required}]` form, added in 2.24. */
export const COMPOSE_MIN = { major: 2, minor: 24 } as const;
/** node:sqlite needs Node 22.13. */
export const NODE_MIN = { major: 22, minor: 13 } as const;

/** HTTP statuses that mean an LLM key was refused. */
export const Status = {
  /** The key is wrong. */
  UNAUTHORIZED: 401,
  /** The key may not use this endpoint. */
  FORBIDDEN: 403,
} as const;
