/** Reuse the complete first-script/worker fixture with hardware-concurrency policy enabled. */
process.env.OYA_CHECK_NATIVE_HARDWARE = '1';
await import('./native-timezone.mjs');
