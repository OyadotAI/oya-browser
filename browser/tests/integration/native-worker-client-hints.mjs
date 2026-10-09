/** Exercise actual worker traffic without overriding native client-hint permission checks. */
process.env.OYA_CHECK_WORKER_HINTS = '1';
await import('./native-client-hints.mjs');
