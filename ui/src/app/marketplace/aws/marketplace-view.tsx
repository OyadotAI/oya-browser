/** A focused account completion card makes pending activation and the selected account explicit. */
'use client';
import { AuthShell } from '@/components/auth/auth-shell';
import { CONTENT } from './content';
import { SetupActions } from './actions';
import { SUPPORT_URL } from './constants';
import { useMarketplace } from './use-marketplace';

/** Account identity wraps completely, so buyers can verify where they are connecting a purchase. */
function AccountSummary({
  email,
  suffix,
}: {
  /** Chosen Oya account. */ email: string;
  /** Trusted AWS suffix. */ suffix?: string;
}) {
  return (
    <dl className="mt-6 space-y-3 text-sm">
      <div>
        <dt className="text-text-muted">Oya account</dt>
        <dd className="mt-1 break-all text-text">{email}</dd>
      </div>
      {suffix && (
        <div>
          <dt className="text-text-muted">AWS account</dt>
          <dd className="mt-1 text-text">•••• {suffix}</dd>
        </div>
      )}
    </dl>
  );
}

/** Every response stays within the existing auth shell, including loading and error recovery. */
export function MarketplaceView() {
  const model = useMarketplace();
  const { auth, state, heading } = model;
  const [title, body] = CONTENT[state.kind];
  const showAccount = ['ready', 'pending'].includes(state.kind) && auth.user;
  const failed = ['missing', 'expired', 'conflict', 'changed', 'unavailable', 'offline'].includes(state.kind);
  return (
    <AuthShell
      glow="accent"
      footer={
        <a href={SUPPORT_URL} className="font-medium text-accent hover:text-accent-hover">
          Contact support
        </a>
      }
    >
      <main>
        <p className="mb-3 text-xs font-medium text-text-muted">AWS Marketplace</p>
        <h1 ref={heading} tabIndex={-1} className="mb-2 font-display text-2xl text-text">
          {title}
        </h1>
        <p className="text-sm text-text-muted" role={failed ? 'alert' : 'status'}>
          {body}
        </p>
        {showAccount && <AccountSummary email={auth.user!.email} suffix={state.awsAccount} />}
        <div className="mt-6 flex flex-col gap-3" aria-busy={state.busy}>
          <SetupActions {...model} />
        </div>
      </main>
    </AuthShell>
  );
}
