# Native session policy subset

`NativeSessionPolicies` is the preparation/verification boundary for the native
**timezone, locale/language, processor-count, platform and User-Agent subset**. It is
not a replacement for the complete persona protector and does not enable native
browsing by default.

One owner must live for the application lifecycle. Before creating any surface:

1. Pass exactly `timeZone`, `locale`, `hardwareConcurrency`, `languages` and
   `platform`, `userAgent` and `userAgentMetadata` to `configure(session, policy)`. Unsupported fields fail instead of being ignored.
   Platform is required and must be exactly `MacIntel`, `Win32` or `Linux x86_64`;
   no implicit host default or string coercion is accepted.
2. Preflight canonicalizes bounded input and requires the engine's versioned native
   state readback plus all setters, including platform, UA and metadata setters. An already-started
   session is rejected.
3. Native setters install timezone, count, locale, platform, UA, then metadata. Readback must match every value
   while the session is still cold before the binding is published as installed.
4. Call `assertConfigured(session)` at the exposure boundary. This certifies only
   this subset; UA strings/metadata coherence, other pre-script protections and
   egress/permissions must independently succeed before any protected browsing surface is exposed.

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

Platform and metadata readback may be absent before installation, but must match the requested
value after the final setter. A missing, malformed or different result quarantines
the session just like a native exception. Private-context integration also verifies
that platform or metadata setter failure never publishes a context and clears its cookies without
pretending to roll back immutable engine identity.

## Native UA and metadata contract

UA strings must be nonempty printable ASCII, at most 1024 characters. Metadata
requires the engine's exact eleven fields, strict booleans, bounded printable
strings, ordered unique matching brand names, and supported unique form factors.
Sparse lists and hidden/unknown fields fail preflight. All arrays and brand pairs
are copied and deeply frozen; native readback is normalized without relying on
object property order. No page shims, header interception or CDP calls are used.

These checks certify native installation and shape, **not semantic persona
coherence**: the policy builder still needs to ensure the UA version, brands,
legacy platform, metadata OS/architecture and the rest of the device agree.
The native first-script fixture uses synthetic Oya identities in parallel sessions
and compares page/frame/worker getters with actual script-request UA headers,
including cold service-worker restart. The default protection path and production
native-browsing guards remain unchanged.
