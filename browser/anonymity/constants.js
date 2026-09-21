/**
 * Every number the Node-side anonymity code uses, by name. The page-side
 * scripts (stealth.js, fingerprint.js, inject.js) keep their own literals: their
 * source is what a visited page sees.
 */

/** A proxy URL without a port means the scheme's default. */
const DEFAULT_PORT = { https: 443, http: 80 };

/** Where the local byte-counting proxy listens: loopback only. */
const LOOPBACK = '127.0.0.1';

/** Indentation of a saved profile, so the JSON stays readable on disk. */
const PROFILE_JSON_INDENT = 2;

module.exports = { DEFAULT_PORT, LOOPBACK, PROFILE_JSON_INDENT };
