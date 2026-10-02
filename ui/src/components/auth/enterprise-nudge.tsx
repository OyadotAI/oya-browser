/**
 * The card under the sign-in and sign-up forms that points teams at the
 * enterprise path of sign-up, so both pages lead to the same sales call.
 */
import Link from 'next/link';

/** Where the enterprise path starts. */
export const ENTERPRISE_SIGNUP = '/signup?plan=enterprise';

/** "Rolling Oya out across a team?", linking to the sales questions. */
export function EnterpriseNudge() {
  return (
    <Link
      href={ENTERPRISE_SIGNUP}
      data-track="cta_clicked"
      data-track-label="auth_enterprise_nudge"
      className="mt-6 flex w-full items-center gap-3 rounded-xl border border-accent/35 bg-accent/5 px-4 py-3.5 text-left"
    >
      <span className="flex-1">
        <span className="block text-sm font-semibold text-text">Rolling Oya out across a team?</span>
        <span className="block text-xs text-text-muted">
          Self-hosting, HIPAA workloads, or more than 5 concurrent browsers? Book a call with the founders.
        </span>
      </span>
      <span className="whitespace-nowrap text-sm font-semibold text-accent">Talk to sales →</span>
    </Link>
  );
}
