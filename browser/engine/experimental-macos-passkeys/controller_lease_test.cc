/** Hermetic controller ownership tests never create an Apple controller or
 * authentication sheet. */
#include "oya_browser_controller_lease.h"

#include <cassert>
#include <iostream>
#include <limits>

using oya::webauthn::ControllerLease;

/** A missing terminal acknowledgment retains one busy slot without accumulating
 * controllers. */
void CancellationQuarantines() {
  ControllerLease owner;
  const auto token = owner.Acquire().value();
  assert(owner.Revoke(token));
  assert(!owner.AllowsResult(token));
  assert(owner.busy() && !owner.Acquire());
  assert(!owner.Revoke(token));
  assert(owner.Complete(token) && !owner.busy());
}

/** Rejected competing requests get no cancellation authority over the active
 * owner. */
void BusyRequestCannotCancelOwner() {
  ControllerLease owner;
  const auto token = owner.Acquire().value();
  assert(!owner.Acquire());
  assert(owner.AllowsResult(token));
  assert(!owner.Revoke(token + 1));
  assert(owner.AllowsResult(token));
}

/** Saved cancellation and duplicate terminal callbacks cannot affect a later
 * controller. */
void OldCallbacksCannotAffectNewOwner() {
  ControllerLease owner;
  const auto old = owner.Acquire().value();
  assert(owner.Complete(old));
  const auto current = owner.Acquire().value();
  assert(current != old);
  assert(!owner.Revoke(old) && !owner.Complete(old));
  assert(owner.AllowsResult(current) && owner.busy());
  assert(owner.Complete(current));
}

/** An operation that completes synchronously can retire before its cancellation
 * handle is returned. */
void SynchronousCompletionCannotBorrowNextOwner() {
  ControllerLease owner;
  const auto completed = owner.Acquire().value();
  assert(owner.Complete(completed));
  const auto next = owner.Acquire().value();
  assert(!owner.Revoke(completed));
  assert(owner.AllowsResult(next));
}

/** Exhaustion fails closed instead of reusing an old token after integer
 * wraparound. */
void TokenExhaustionNeverReusesIdentity() {
  ControllerLease owner(std::numeric_limits<std::uint64_t>::max() - 1);
  const auto last = owner.Acquire().value();
  assert(owner.Complete(last));
  assert(!owner.Acquire() && !owner.busy());
}

/** These state scenarios are not proof of actual OS terminal callback behavior.
 */
int main() {
  CancellationQuarantines();
  BusyRequestCannotCancelOwner();
  OldCallbacksCannotAffectNewOwner();
  SynchronousCompletionCannotBorrowNextOwner();
  TokenExhaustionNeverReusesIdentity();
  std::cout << "PASS controller ownership states; Apple controller integration "
               "unverified\n";
}
