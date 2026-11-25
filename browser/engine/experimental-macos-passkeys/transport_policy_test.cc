/** Portable transport tests exercise original ID restrictions without invoking
 * Apple, Bluetooth or credentials. */
#include "oya_browser_transport_policy.h"

#include <array>
#include <cassert>
#include <iostream>

using oya::webauthn::BrowserCredentialRule;
using oya::webauthn::BrowserTransport;
using oya::webauthn::BrowserTransportPolicy;

/** Empty allow-lists support local or phone choice according to the original
 * global transport set. */
void EmptyAllowListRespectsGlobalTransports() {
  const std::array<std::uint8_t, 1> id{1};
  BrowserTransportPolicy both(true, true, {});
  assert(both.DiscoveryTransport() == BrowserTransport::kInternal);
  assert(both.AllowsResult(id, BrowserTransport::kInternal));
  assert(both.AllowsResult(id, BrowserTransport::kHybrid));
  BrowserTransportPolicy phone(false, true, {});
  assert(phone.DiscoveryTransport() == BrowserTransport::kHybrid);
  assert(!phone.AllowsResult(id, BrowserTransport::kInternal));
  assert(phone.AllowsResult(id, BrowserTransport::kHybrid));
}

/** Unspecified descriptor transports permit both; internal+hybrid is equivalent
 * for the native bridge. */
void UnspecifiedAndCombinedPermissions() {
  const BrowserCredentialRule unspecified{{1}, true, true};
  BrowserTransportPolicy policy(true, true, {unspecified});
  assert(policy.IncludeCredential(0));
  assert(policy.AllowsResult(unspecified.credential_id,
                             BrowserTransport::kInternal));
  assert(policy.AllowsResult(unspecified.credential_id,
                             BrowserTransport::kHybrid));
}

/** Internal-only and USB-only lists cannot trigger the experimental phone
 * provider. */
void NonHybridListsDoNotLaunch() {
  BrowserTransportPolicy internal(true, true, {{{1}, true, false}});
  assert(!internal.DiscoveryTransport());
  BrowserTransportPolicy usb(true, true, {{{1}, false, false}});
  assert(!usb.DiscoveryTransport() && !usb.IncludeCredential(0));
  BrowserTransportPolicy global_denial(true, false, {});
  assert(!global_denial.DiscoveryTransport());
}

/** A hybrid-only allow-list never accepts an OS-local choice for that ID. */
void HybridOnlyRejectsLocalSelection() {
  const BrowserCredentialRule phone{{1}, false, true};
  BrowserTransportPolicy policy(false, true, {phone});
  assert(policy.DiscoveryTransport() == BrowserTransport::kHybrid);
  assert(policy.AllowsResult(phone.credential_id, BrowserTransport::kHybrid));
  assert(
      !policy.AllowsResult(phone.credential_id, BrowserTransport::kInternal));
}

/** A credential cannot borrow the phone permission granted to a different ID in
 * the same request. */
void MixedIdsKeepTheirOwnPermissions() {
  const BrowserCredentialRule local{{1}, true, false}, phone{{2}, false, true},
      usb{{3}, false, false};
  BrowserTransportPolicy policy(true, true, {local, phone, usb});
  assert(policy.IncludeCredential(0) && policy.IncludeCredential(1));
  assert(!policy.IncludeCredential(2) && !policy.IncludeCredential(3));
  assert(policy.AllowsResult(local.credential_id, BrowserTransport::kInternal));
  assert(!policy.AllowsResult(local.credential_id, BrowserTransport::kHybrid));
  assert(policy.AllowsResult(phone.credential_id, BrowserTransport::kHybrid));
  assert(
      !policy.AllowsResult(phone.credential_id, BrowserTransport::kInternal));
  assert(!policy.AllowsResult(usb.credential_id, BrowserTransport::kHybrid));
}

/** Global filtering omits local-only IDs when the validated native intersection
 * forbids internal transport. */
void HybridRoleOmitsLocalOnlyIds() {
  BrowserTransportPolicy policy(false, true,
                                {{{1}, true, false}, {{2}, false, true}});
  assert(!policy.IncludeCredential(0) && policy.IncludeCredential(1));
}

/** Empty IDs, unknown IDs and unrepresentable attachments cannot authorize
 * native success. */
void InvalidReturnsAreDenied() {
  BrowserTransportPolicy policy(true, true, {{{1}, true, true}});
  const std::array<std::uint8_t, 1> unknown{2};
  assert(!policy.AllowsResult({}, BrowserTransport::kHybrid));
  assert(!policy.AllowsResult(unknown, BrowserTransport::kHybrid));
  const std::array<std::uint8_t, 1> known{1};
  assert(!policy.AllowsResult(known, static_cast<BrowserTransport>(-1)));
}

/** Native protocol rules are checked here; full FIDO filtering and Apple
 * attachment tests remain separate. */
int main() {
  EmptyAllowListRespectsGlobalTransports();
  UnspecifiedAndCombinedPermissions();
  NonHybridListsDoNotLaunch();
  HybridOnlyRejectsLocalSelection();
  MixedIdsKeepTheirOwnPermissions();
  HybridRoleOmitsLocalOnlyIds();
  InvalidReturnsAreDenied();
  std::cout << "PASS native transport policy; request-handler and Apple "
               "integration unverified\n";
}
