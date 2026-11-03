# Isolated native phone-passkey integration

This is source for an incomplete, separately gated macOS experiment. It is
**not a compiled engine or a runnable phone-passkey fix**. Production desktop
packages continue to use stock Electron. These patches do not enable the
associated-domain `platformPasskeys` provider or change a website's RP/origin.

The source is pinned to Electron **45.2.1**, commit
`f81ca3d4f09f57667238120adacb01239d5ecd89`, and Chromium **156.0.8078.75**,
commit `861389936bf33ea77529ee5dcc0605e68f6bb620`. The historical 44.5.1
macOS patches are not a compatible test artifact.

## Source implementation

Apply these patches only in a matching, complete Electron build checkout,
after applying Electron's upstream patch stack:

1. `chromium-request-policy.patch`, from the Chromium source root. It adds
   native start/final-success admission, exact request/document/context/window
   identity, one-use attempt decisions, deferred cancellation/retirement, and
   guards against callbacks from old attempts.
2. `chromium-browser-provider.patch`, from the Chromium source root, after the
   first patch. It connects the browser-specific Apple discovery and assertion
   controller, original per-credential transport restrictions, native automatic
   dispatch authorization and OS-owned hybrid transport metadata.
3. `electron-browser-passkeys.patch`, from the Electron source root. It adds
   the private main-process capability, policy-event and retirement bridge.

The Chromium changes also have to coexist with Electron's upstream Chromium
patch stack. Selected-file application checks do not establish compatibility
with the whole stack, a complete build, linking or runtime behavior.

The new provider exclusively handles eligible, opted-in modal hybrid assertions.
It disables duplicate Touch ID/iCloud discovery only for that attempt. Ordinary
launches, internal-only requests, registration and conditional requests retain
their existing provider. Same-ceremony choice between both credential stores is
outside this experiment.

The native provider snapshots each original credential ID's transport hints
before handler specialization. It filters unsupported descriptors, uses a real
hybrid discovery for hybrid-only requests, and verifies the actual Apple
attachment and returned ID against that same original credential's permissions.
An OS-managed hybrid discovery skips the browser's entire Bluetooth setup path;
it does not claim that Bluetooth is on or that Apple has permission. Mixed
browser/OS routes preserve the native route when the browser adapter is absent.

The controller has one owner. Cancel revokes its result immediately and keeps
the slot quarantined until Apple's terminal callback. Old cancellation handles
cannot affect a new owner. Missing terminal callbacks retain one bounded slot
until application restart. Request retirement does not release that native slot.

## Private engine contract

The main-process adapter activates only with `--oya-native-passkeys` and
`app.getOyaBrowserPasskeyCapabilities()` returning
`{ protocolVersion: 1, enabled: true }`. The native method additionally requires
supported macOS, a genuine process browser entitlement and the associated-domain
provider disabled. An unchanged stock engine cannot enable the experiment.

The session `oya-webauthn-request` event contains only
`{ requestId, phase, frame, relyingPartyId, origin, kind, presentation, transports }`,
a boolean decision callback and a persistent cancellation closure. `phase` is
`start` or `complete`; `kind` is `get` and `presentation` is `modal`. Assertions,
challenges and credential IDs never cross this JavaScript policy boundary.

Decisions only record a boolean; native dispatch must finish before discovery or
page completion. Native checks run again before OS perform and final success.
Profile/control/recording changes revoke the main-process lease. The deferred
`oya-webauthn-ended` event carries `{ requestId, outcome: 'retired' }`; retirement
is not a claim of successful authentication or an OS terminal callback.

Experimental account-choice events carry the matching `requestId`. The Apple
browser provider uses Apple's account selection instead of a competing chooser.

Firefox is the behavior reference: its
[native service](https://searchfox.org/firefox-main/source/dom/webauthn/MacOSWebAuthnService.mm)
uses Apple's browser credential flow and hybrid option. This implementation
extends Chromium's compatible bridge; no Gecko source or Mozilla signing
identity is copied. Oya must use its own Developer ID and provisioning profile.

## Verification

Portable tests invoke no browser, credential provider, biometric or Bluetooth UI:

```sh
for test in policy_state controller_lease transport_policy; do
  xcrun clang++ -std=c++20 -Wall -Wextra -Werror -pedantic \
    "browser/engine/experimental-macos-passkeys/${test}_test.cc" \
    -o "/private/tmp/oya-${test}-test" || exit
  "/private/tmp/oya-${test}-test" || exit
done
```

All three portable executables passed on November 2, 2026. They cover native
policy dispatch, controller ownership/quarantine and original per-ID transport
restrictions. They do not compile Chromium, Objective-C or the Node/V8 bridge.
The applied provider patch's portable headers match these tested copies.

The native patches additionally contain six Common request-flow fixtures, eight
FIDO handler fixtures and eight Apple controller/discovery fixtures. These are
**authored, not compiled or run**. Apple test hooks intercept all perform/cancel
actions; a missing hook fails closed instead of opening authentication UI.

The two Chromium patches passed ordered clean apply/reverse checks against the
selected pinned source files, and the Electron patch passed its pinned selected
source apply/reverse checks. All 23 selected Chromium baseline files matched
the retrieved full pinned main checkout byte-for-byte. A separate scratch
fixture applied all 188 upstream Electron Chromium patches, followed by both
custom Chromium patches, across 677 paths. This verifies textual compatibility
with that Chromium patch series; it does not compile the engine or verify other
upstream repository patch series.

The application unit suite passed 3,541 tests again on November 2, 2026, with
lint, TypeScript, formatting and app build checks passing. The first four
integration scripts passed: regression, control-state, release-publication and
workflow. These checks used stock dependencies. Loopback tests required execution
outside the sandbox; remaining native runtime checks require `OYA_NATIVE_ENGINE`.

Correctness review found and repaired a caught-duplicate decision callback bug.
The repaired source then had zero open correctness, security or reliability
findings. QA reran 19 app seam tests and the three portable executables against
freshly applied headers, and verified patch integrity. QA's verdict remains
**do not ship as a passkey fix** because native and physical gates are open.

## Release gates

Completion requires full engine compilation/link, compiled native fixtures,
actual Node/V8 event-boundary tests, signed packaging and launch, and physical
Google first/repeated phone sign-in, cancellation/retry and owner-change checks.
No existing running app contains this integration.

The full pinned Chromium main checkout was retrieved locally, but dependency
sync failed on repeated upstream HTTP 400/500 responses for pinned helper/data
revisions. The attempt did not reach successful dependency sync or compilation.
The available branch heads are not substitutes for the pinned revisions.

The signing preflight rejects invalid, expired, wrong-team/wrong-app profiles,
unauthorized keychain groups and missing browser capability. Both the installed
and downloaded Oya profiles were rechecked on November 2, 2026: neither contains
`com.apple.developer.web-browser.public-key-credential`. Apple's portal approval
status is unknown. A valid Developer ID certificate alone is insufficient.
Do not add the entitlement to an unmatched profile or bypass macOS validation.

The maintainer authorized commit, push and release when the work is complete.
Until these gates pass, keep the PR draft and do not publish this source as a
working passkey release or replace the installed app.
