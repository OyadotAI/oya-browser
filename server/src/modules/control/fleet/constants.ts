/** Every number and fixed name the fleet runtimes run on. */

/** Default port of an https: egress proxy URL. */
export const HTTPS_PORT = 443;
/** Default port of an http: egress proxy URL. */
export const HTTP_PORT = 80;
/** Random bytes in a session's enrollment and egress token. */
export const TOKEN_BYTES = 32;

/** Longest a docker or kubectl call may run. */
export const CLI_TIMEOUT_MS = 30_000;
/** Most output read back from a docker call (1 MiB). */
export const MAX_DOCKER_OUTPUT = 1_048_576;

/** Kubernetes names and label values are at most this long. */
export const K8S_NAME_MAX = 63;
/** A too-long pod name keeps this much of its slug… */
export const POD_SLUG_KEEP = 50;
/** …followed by this many hex characters of the id's digest. */
export const POD_DIGEST_CHARS = 12;
/** The app label every managed pod carries, which the NetworkPolicy must select. */
export const POD_LABEL = 'oya-managed-browser';
/** The namespace when OYA_K8S_NAMESPACE is unset. */
export const DEFAULT_NAMESPACE = 'oya-browsers';

/** What a verified managed runtime can do. */
export const CAPABILITIES = ['egress', 'persona', 'terminate', 'takeover'];
