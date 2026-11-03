/** Preserve original per-credential transport restrictions across the Apple
 * browser authentication flow. */
#ifndef OYA_BROWSER_TRANSPORT_POLICY_H_
#define OYA_BROWSER_TRANSPORT_POLICY_H_

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>
#include <utility>
#include <vector>

namespace oya::webauthn {

/** Only verified Apple platform or nearby-phone attachments can produce a
 * result. */
enum class BrowserTransport { kInternal, kHybrid };

/** Native-only original descriptor; no credential identifier is sent to
 * JavaScript policy events. */
struct BrowserCredentialRule {
  /** Exact credential bytes supplied by the relying party. */
  std::vector<std::uint8_t> credential_id;
  /** Unspecified descriptor transports permit both of these native methods. */
  bool internal_allowed = false;
  /** A different credential's hybrid permission cannot authorize this ID. */
  bool hybrid_allowed = false;
};

/** An immutable native snapshot combines RP-intersected global transport policy
 * and individual ID rules. */
class BrowserTransportPolicy {
 public:
  /** The caller derives global permissions from the original validated
   * transport intersection. */
  BrowserTransportPolicy(bool internal_allowed,
                         bool hybrid_allowed,
                         std::vector<BrowserCredentialRule> rules)
      : internal_allowed_(internal_allowed),
        hybrid_allowed_(hybrid_allowed),
        rules_(std::move(rules)) {}

  /** Hybrid-only requests use a genuine hybrid discovery rather than injecting
   * internal transport. */
  std::optional<BrowserTransport> DiscoveryTransport() const {
    if (!hybrid_allowed_ || !HasHybridCredential())
      return std::nullopt;
    return internal_allowed_ ? BrowserTransport::kInternal
                             : BrowserTransport::kHybrid;
  }

  /** Unsupported IDs are omitted from the OS request without widening any
   * original descriptor. */
  bool IncludeCredential(std::size_t index) const {
    return index < rules_.size() &&
           ((internal_allowed_ && rules_[index].internal_allowed) ||
            (hybrid_allowed_ && rules_[index].hybrid_allowed));
  }

  /** An actual returned ID and attachment must satisfy the same original
   * credential's restrictions. */
  bool AllowsResult(std::span<const std::uint8_t> credential_id,
                    BrowserTransport actual) const {
    if (credential_id.empty() || !GlobalAllows(actual))
      return false;
    if (rules_.empty())
      return true;
    return std::ranges::any_of(rules_, [&](const auto& rule) {
      return std::ranges::equal(rule.credential_id, credential_id) &&
             ((actual == BrowserTransport::kInternal &&
               rule.internal_allowed) ||
              (actual == BrowserTransport::kHybrid && rule.hybrid_allowed));
    });
  }

 private:
  /** An empty allow-list permits discovery; a nonempty one needs its own
   * hybrid-eligible entry. */
  bool HasHybridCredential() const {
    return rules_.empty() || std::ranges::any_of(rules_, [](const auto& rule) {
             return rule.hybrid_allowed;
           });
  }

  /** Unknown native attachments fail closed rather than being treated as a
   * phone. */
  bool GlobalAllows(BrowserTransport actual) const {
    return (actual == BrowserTransport::kInternal && internal_allowed_) ||
           (actual == BrowserTransport::kHybrid && hybrid_allowed_);
  }

  /** Actual RP-intersected internal permission, not inferred from another
   * allowed ID. */
  const bool internal_allowed_;
  /** Actual RP-intersected hybrid permission. */
  const bool hybrid_allowed_;
  /** Immutable descriptor snapshot belongs only to this native assertion
   * attempt. */
  const std::vector<BrowserCredentialRule> rules_;
};

}  // namespace oya::webauthn
#endif  // OYA_BROWSER_TRANSPORT_POLICY_H_
