/** Exercise the native policy coordinator on real Oya first-script page and worker surfaces. */
process.env.OYA_CHECK_NATIVE_POLICY = '1';
await import('./native-locale.mjs');
