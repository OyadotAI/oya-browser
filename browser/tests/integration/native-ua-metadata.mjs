/** Exercise native metadata with the same real first-script and worker-restart fixtures as user-agent strings. */
process.env.OYA_CHECK_NATIVE_METADATA = '1';
await import('./native-user-agent.mjs');
