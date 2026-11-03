/** Keep one native authentication controller quarantined until its exact
 * terminal callback. */
#ifndef OYA_BROWSER_CONTROLLER_LEASE_H_
#define OYA_BROWSER_CONTROLLER_LEASE_H_

#include <cstdint>
#include <limits>
#include <optional>

namespace oya::webauthn {

/** Cancellation revokes results immediately but never pretends the OS
 * controller has ended. */
class ControllerLease {
 public:
  /** A test can seed exhaustion; production starts at zero and never reuses a
   * token. */
  explicit ControllerLease(std::uint64_t initial_sequence = 0)
      : sequence_(initial_sequence) {}
  ControllerLease(const ControllerLease&) = delete;
  ControllerLease& operator=(const ControllerLease&) = delete;

  /** Busy or exhausted owners refuse another controller without replacing
   * current storage. */
  std::optional<std::uint64_t> Acquire() {
    if (active_ || sequence_ == std::numeric_limits<std::uint64_t>::max())
      return std::nullopt;
    active_ = ++sequence_;
    revoked_ = false;
    return active_;
  }

  /** Only the original operation may send cancellation, and only once. */
  bool Revoke(std::uint64_t token) {
    if (!IsCurrent(token) || revoked_)
      return false;
    revoked_ = true;
    return true;
  }

  /** An old controller's late terminal callback cannot clear a new controller's
   * slot. */
  bool Complete(std::uint64_t token) {
    if (!IsCurrent(token))
      return false;
    active_.reset();
    revoked_ = false;
    return true;
  }

  /** Identity remains current during cancellation quarantine. */
  bool IsCurrent(std::uint64_t token) const {
    return active_ && *active_ == token;
  }

  /** A success racing cancellation is unusable even before terminal cleanup
   * runs. */
  bool AllowsResult(std::uint64_t token) const {
    return IsCurrent(token) && !revoked_;
  }

  /** Lack of an OS terminal acknowledgment keeps capacity at exactly one. */
  bool busy() const { return active_.has_value(); }

 private:
  /** Tokens never wrap or transfer across operations. */
  std::uint64_t sequence_;
  /** The only operation whose native controller storage may be touched. */
  std::optional<std::uint64_t> active_;
  /** Main/request timeout does not release OS ownership. */
  bool revoked_ = false;
};

}  // namespace oya::webauthn
#endif  // OYA_BROWSER_CONTROLLER_LEASE_H_
