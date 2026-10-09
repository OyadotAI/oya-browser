# Native session policy subset

`NativeSessionPolicies` is the preparation/verification boundary for the native
**timezone, locale/language and processor-count subset**. It is not a replacement
for the complete persona protector and does not enable native browsing by default.

One owner must live for the application lifecycle. Before creating any surface:

1. Pass exactly `timeZone`, `locale`, `hardwareConcurrency` and `languages` to
   `configure(session, policy)`. Unsupported fields fail instead of being ignored.
2. Preflight canonicalizes bounded input and requires the engine's versioned native
   state readback plus all setters. An already-started session is rejected.
3. Native setters install the immutable values. Readback must match every value
   while the session is still cold before the binding is published as installed.
4. Call `assertConfigured(session)` at the exposure boundary. This certifies only
   this subset; UA metadata, other pre-script protections and egress/permissions
   must independently succeed before any protected browsing surface is exposed.

Native setters are **not transactional**. An engine error or mismatched readback
can leave partial native state. The owner permanently rejects reuse of that exact
session; retire it and allocate a different partition. Do not create another owner
to retry the same session or bypass this owner by constructing a window directly.
Input/capability preflight failures occur before setter calls and do not publish a
binding. Identical canonical input reuses an installed binding without mutation;
a different persona cannot repurpose it. Returned policies and language arrays
are frozen copies, not references into the caller's input.

The readback revision requires the native language extension, not merely similarly
named locale methods on an older engine. No debugger, protocol adapter, request
interception, page getter shim or fallback is used. The default application path
is deliberately unchanged until the rest of the persona migration is complete.

`npm run test:native-policy` (with explicit `OYA_NATIVE_ENGINE`) runs this source
against first-script page/frame/worker fixtures, actual request headers, native
first-renderer locks and a real partially failed installation. Unit tests cover
preflight, immutable snapshots, reentrancy, missing capabilities, native readback
mismatches and failure at each setter.
