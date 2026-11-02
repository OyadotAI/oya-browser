# Isolated native phone-passkey integration

This is preparatory code for a separately authorized macOS test build. It is
**not a completed engine patch or a runnable phone-passkey fix**. Production
desktop packages continue to use stock Electron. Nothing here enables the
associated-domain `platformPasskeys` provider or changes a website's RP/origin.

The proposed native integration is pinned to Electron **45.2.1**, commit
`f81ca3d4f09f57667238120adacb01239d5ecd89`, and its Chromium **156.0.8078.75**,
commit `861389936bf33ea77529ee5dcc0605e68f6bb620`. The historical 44.5.1 macOS
patches are not a compatible or current test artifact.

## Implemented and checked

- The main-process adapter requires the explicit `--oya-native-passkeys` switch
  and native capability protocol version 1. An unchanged stock runtime is
  refused. Ordinary launches create no experimental policy handlers.
- Private session policy events carry only attempt identity, native frame,
  validated RP/origin, presentation and transport categories. Start and final
  success have separate decisions. The native cancellation closure remains
  usable after start authorization; account selection cannot borrow another
  attempt. Profile/control/recording changes revoke the original lease.
- The signing preflight rejects missing/expired/wrong-team/wrong-app profiles,
  missing browser-credential capability and unauthorized keychain groups before
  adding the restricted entitlement. A public profile's metadata is not proof
  that the final executable can launch; signature and actual launch remain gates.
- `oya_passkey_policy_state.h` implements the portable native dispatch state.
  Its executable tests cover duplicate/late decisions, allow-then-throw, native
  identity changes, persistent cancellation, reentrancy and final success once.
  It is not yet connected to Chromium's request or Apple discovery.

Run the portable native tests from the repository root:

```sh
xcrun clang++ -std=c++20 -Wall -Wextra -Werror -pedantic \
  browser/engine/experimental-macos-passkeys/policy_state_test.cc \
  -o /private/tmp/oya-passkey-policy-state-test
/private/tmp/oya-passkey-policy-state-test
```

These tests invoke no browser, credential provider, biometric or Bluetooth UI.

## Required engine contract

The paired native implementation must provide:

1. `app.getOyaBrowserPasskeyCapabilities()` returning
   `{ protocolVersion: 1, enabled: true }` only when the isolated experiment,
   supported OS and genuine process entitlement are active. Capability metadata
   must not expose credentials or assertion contents.
2. A private session `oya-webauthn-request` event with
   `{ requestId, phase: 'start' | 'complete', frame, relyingPartyId, origin,
kind: 'get', presentation: 'modal', transports }`, a decision callback and a
   persistent native cancellation closure. The final event must run at central
   successful assertion delivery, including OS-preselected single accounts.
3. Both native gates must close dispatch before discovery/page completion and
   recheck the exact RequestState, attempt, live native document and owner.
   Decision callbacks only record a boolean. Cancellation tombstones before
   posting Chromium cleanup. Old callbacks cannot address a new request.
4. `oya-webauthn-ended` with `{ requestId, outcome }` retires the app lease.
   Native OS-controller ownership remains quarantined until its own terminal
   callback; request retirement alone cannot free that controller slot.
5. Experimental Touch ID account-choice events carry the matching `requestId`.
   The Apple browser provider uses its own account selection rather than a
   competing Oya chooser.

Reuse Chromium's browser-specific iCloud/Apple bridge after native RP/origin and
request validation. Do not replace `navigator.credentials` or forward assertions
through JavaScript/agent APIs. For get-only additional discovery, preserve the
RP-intersected transport set: hybrid-only must use a hybrid discovery and matching
authenticator options, rather than injecting internal transport. The actual OS
attachment and returned credential ID must satisfy that credential's original
transport restrictions. A single native browser-provider controller must be used.

For the narrow phone experiment, browser-provider discovery is exclusive for an
eligible hybrid assertion; disable only that attempt's Touch ID discovery to
avoid two sheets. Ordinary builds, registration and internal-only requests retain
their existing provider. Same-ceremony choice between both credential stores is
not implemented by this preparatory code.

## Remaining gates

The engine integration, hybrid-only and mixed-credential transport tests,
controller cancellation/quarantine tests, engine compilation/link, signed
packaging/launch, and physical Google phone sign-in have not run or passed.
There is no completed native integration patch in this directory yet.

A full build also needs adequate build space and a matching Apple profile with
`com.apple.developer.web-browser.public-key-credential`. Neither the installed
profile nor the existing downloaded Oya profile currently contains that
capability. This does not establish Apple's portal approval status. Do not add
the entitlement to an unmatched profile or bypass macOS validation.

Only an actual signed, launched artifact and first/repeated physical Google
sign-in, cancel/retry and owner-change checks can establish the reported fix.
Portable state tests and the application fake-native tests do not establish OS
phone support. No release or installed-app replacement is authorized here.

## Preparatory verification

On November 2, 2026, the application unit suite passed 3,541 tests. Lint,
TypeScript checking, formatting and the application build passed. The standalone
C++ policy test compiled with warnings treated as errors and passed. These
checks used stock build dependencies, not a patched engine.

The integration runner passed regression, control-state, release-publication and
workflow checks, then stopped at the analyzer check because `OYA_NATIVE_ENGINE`
was unavailable. The remaining native checks did not run. Loopback-dependent
checks required execution outside the filesystem/network sandbox.
