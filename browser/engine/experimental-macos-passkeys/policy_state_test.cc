/** Hermetic native policy-state tests do not invoke a browser, authenticator, OS sheet or credential provider. */
#include "oya_passkey_policy_state.h"

#include <cassert>
#include <iostream>

using oya::webauthn::PolicyPhase;
using oya::webauthn::PolicyState;

/** Start authorization is explicit and distinct from final result authorization. */
void SeparateSuccessGate() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  state.Decide(start, true);
  assert(state.Finish(start, true, true, true) && state.active());
  const auto complete = state.Begin(PolicyPhase::kComplete).value();
  assert(!state.Finish(complete, true, true, true));
  assert(!state.completed());
}

/** A successful OS-preselected account still requires exactly one final native policy decision. */
void OneFinalSuccess() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  state.Decide(start, true);
  assert(state.Finish(start, true, true, true));
  const auto final = state.Begin(PolicyPhase::kComplete).value();
  state.Decide(final, true);
  assert(state.Finish(final, true, true, true) && state.completed());
  assert(!state.Finish(final, true, true, true));
  assert(!state.Begin(PolicyPhase::kComplete));
}

/** Saved admission callbacks do not authorize a newer dispatch, even on the same native request attempt. */
void OldCallbackCannotComplete() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  state.Decide(start, true);
  assert(state.Finish(start, true, true, true));
  const auto final = state.Begin(PolicyPhase::kComplete).value();
  state.Decide(start, true);
  assert(!state.Finish(final, true, true, true));
}

/** A handler that records allow and then throws cannot start native authentication. */
void AllowThenThrowIsDenied() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  state.Decide(start, true);
  assert(!state.Finish(start, true, true, false));
  assert(!state.Begin(PolicyPhase::kStart));
}

/** Native document/window/source changes override an otherwise valid main-process allow. */
void NativeIdentityIsRechecked() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  state.Decide(start, true);
  assert(!state.Finish(start, true, false, true));
}

/** No listener, no answer, duplicate answers and post-dispatch answers all deny. */
void MissingOrLateDecisionIsDenied() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  assert(!state.Finish(start, true, true, true));
  state.Decide(start, true);
  assert(!state.active() && !state.Finish(start, true, true, true));
}

/** A duplicate boolean cannot silently preserve an earlier allow. */
void DuplicateDecisionIsDenied() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  state.Decide(start, true);
  state.Decide(start, true);
  assert(!state.Finish(start, true, true, true));
}

/** Lifetime cancellation survives the consumed admission callback and cannot revive on a late OS result. */
void CancellationPersists() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  state.Decide(start, true);
  assert(state.Finish(start, true, true, true));
  state.Revoke();
  assert(!state.active() && !state.Begin(PolicyPhase::kComplete));
  state.Decide(start, true);
  assert(!state.completed());
}

/** A reentrant final phase cannot replace an unresolved admission gate. */
void ReentrantPhaseIsDenied() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  assert(!state.Begin(PolicyPhase::kComplete));
  state.Decide(start, true);
  assert(!state.Finish(start, true, true, true));
}

/** Cancellation during final dispatch wins over a recorded allow before any result may leave native code. */
void CancelDuringFinalGate() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  state.Decide(start, true);
  assert(state.Finish(start, true, true, true));
  const auto final = state.Begin(PolicyPhase::kComplete).value();
  state.Decide(final, true);
  state.Revoke();
  assert(!state.Finish(final, true, true, true));
}

/** Native retirement rejects all retained callbacks and does not transfer policy to another attempt. */
void EndIsTerminal() {
  PolicyState state;
  const auto start = state.Begin(PolicyPhase::kStart).value();
  state.End();
  state.Decide(start, true);
  assert(!state.Finish(start, true, true, true));
  assert(!state.Begin(PolicyPhase::kStart));
}

/** Execute only portable native state transitions; no passing result here proves OS or Google compatibility. */
int main() {
  SeparateSuccessGate();
  OneFinalSuccess();
  OldCallbackCannotComplete();
  AllowThenThrowIsDenied();
  NativeIdentityIsRechecked();
  MissingOrLateDecisionIsDenied();
  DuplicateDecisionIsDenied();
  CancellationPersists();
  ReentrantPhaseIsDenied();
  CancelDuringFinalGate();
  EndIsTerminal();
  std::cout << "PASS native policy states; browser/OS/phone integration unverified\n";
}
