# Product verification

1. Convert the request and change into a risk-ranked test matrix. Cover the real entry point,
   installation/first use, the primary journey, failure and recovery, and state after restart.
   Identify build/version, environment, account type, and fixture setup before reproduction.
2. Use isolated fixtures and separate ports. Inspect what is already running; do not quit the
   person's app, take their port, or spend paid API credits unless the assignment authorizes it.
   Keep credentials, real remotes, production data, and user work out of destructive scenarios.
3. Exercise success, denial, error, timeout, cancellation, disconnect, retry, duplicate and
   simultaneous requests. Probe empty/large inputs, unicode and quoted paths, long names,
   boundary values, expired sessions, and partially completed installation.
4. Test the actual surface: Oya for browser interaction; native app tooling for native windows;
   CLI/API checks for those contracts. A mocked web preview checks rendering only. Identify any
   real integration or action that could not be exercised. Never label fixtures as end-to-end.
5. Measure timing and resource cleanup. After each exit, check persisted state, leaked tasks,
   processes, ports, handles, temporary files, and resources that should have been released.
6. Use `../assets/verification.md`. Give copyable reproduction steps, expected/actual behavior,
   hit rate if flaky, relevant evidence, coverage gaps, and cleanup status. Declare a verdict
   only for the tested scope. Keep hypotheses separate from reproduced bugs.

Run only checks within the assignment's authorization. Create test fixtures in scratch space;
do not modify product code as part of an independent QA pass.
