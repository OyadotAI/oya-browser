/** Exercise native platform values through real Oya first-script document and worker surfaces. */
process.env.OYA_CHECK_NATIVE_PLATFORM = '1';
await import('./native-user-agent.mjs');
