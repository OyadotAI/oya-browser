/** Verify native ICU locale policy together with timezone and hardware isolation, using only the explicit Oya engine. */
process.env.OYA_CHECK_NATIVE_LOCALE = '1';
process.env.OYA_CHECK_NATIVE_HARDWARE = '1';
await import('./native-timezone.mjs');
