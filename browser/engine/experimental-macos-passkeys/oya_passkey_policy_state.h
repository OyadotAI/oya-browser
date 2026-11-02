/** Native policy decisions are one-use and attempt-local; no credential bytes enter this state machine. */
#ifndef OYA_BROWSER_PASSKEY_POLICY_STATE_H_
#define OYA_BROWSER_PASSKEY_POLICY_STATE_H_

#include <cstdint>
#include <optional>

namespace oya::webauthn {

/** Both native admission and final successful assertion delivery need independent policy decisions. */
enum class PolicyPhase { kStart, kComplete };

/** Owned by exactly one native request attempt, never reused for a replacement Chromium RequestState. */
class PolicyState {
 public:
  /** A serial binds each callback to the still-open native dispatch, including reentrant callbacks. */
  std::optional<std::uint64_t> Begin(PolicyPhase phase) {
    if (!CanBegin(phase)) {
      Revoke();
      return std::nullopt;
    }
    phase_ = phase;
    state_ = phase == PolicyPhase::kStart ? State::kStarting : State::kCompleting;
    ResetDecision();
    return ++serial_;
  }

  /** A saved start callback cannot authorize a later complete phase or a closed dispatch window. */
  void Decide(std::uint64_t serial, bool allowed) {
    if (serial != serial_ || !DispatchOpen()) return;
    if (answered_) {
      consistent_ = false;
      allowed_ = false;
      return;
    }
    answered_ = true;
    allowed_ = allowed;
  }

  /** Actual browser dispatch closes before any OS discovery or page callback can proceed. */
  bool Finish(std::uint64_t serial,
              bool had_listener,
              bool native_source_current,
              bool dispatch_succeeded) {
    if (serial != serial_ || !DispatchOpen()) return false;
    const bool allowed = answered_ && consistent_ && allowed_ && had_listener &&
                         native_source_current && dispatch_succeeded;
    SetOutcome(allowed);
    return allowed;
  }

  /** Native cancellation tombstones immediately, before posted Chromium cleanup or an OS callback. */
  void Revoke() {
    if (state_ == State::kCompleted || state_ == State::kEnded) return;
    state_ = State::kRevoked;
    allowed_ = false;
  }

  /** End is native request retirement, not proof that a separate OS controller finished cancellation. */
  void End() { state_ = State::kEnded; }

  /** Final native policy can run only after successful admission and before revocation. */
  bool active() const { return state_ == State::kActive; }

  /** Only one final policy dispatch may authorize the original native result. */
  bool completed() const { return state_ == State::kCompleted; }

 private:
  /** Lifecycle transitions never infer permission from an old callback's boolean. */
  enum class State {
    kFresh,
    kStarting,
    kActive,
    kCompleting,
    kCompleted,
    kRevoked,
    kEnded,
  };

  /** Never let a repeated, out-of-order or reentrant gate create a second policy wait. */
  bool CanBegin(PolicyPhase phase) const {
    return (phase == PolicyPhase::kStart && state_ == State::kFresh) ||
           (phase == PolicyPhase::kComplete && state_ == State::kActive);
  }

  /** Late callbacks have no effect once native event dispatch has closed. */
  bool DispatchOpen() const {
    return state_ == State::kStarting || state_ == State::kCompleting;
  }

  /** A fresh final phase cannot inherit admission's decision or callback count. */
  void ResetDecision() {
    answered_ = false;
    consistent_ = true;
    allowed_ = false;
  }

  /** Denial is permanent for this attempt; native retry creates a distinct attempt and object. */
  void SetOutcome(bool allowed) {
    if (!allowed) {
      state_ = State::kRevoked;
      return;
    }
    state_ = phase_ == PolicyPhase::kStart ? State::kActive : State::kCompleted;
  }

  /** Current native lifecycle. */
  State state_ = State::kFresh;
  /** The active decision phase. */
  PolicyPhase phase_ = PolicyPhase::kStart;
  /** Monotonic per-attempt dispatch identity, never a website-controlled request number. */
  std::uint64_t serial_ = 0;
  /** Exactly one callback must answer within native dispatch. */
  bool answered_ = false;
  /** Repeated decisions invalidate the gate instead of retaining the first allow. */
  bool consistent_ = true;
  /** No default allow exists for missing handlers or failed native dispatch. */
  bool allowed_ = false;
};

}  // namespace oya::webauthn

#endif  // OYA_BROWSER_PASSKEY_POLICY_STATE_H_
