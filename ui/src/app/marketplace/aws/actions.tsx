/** Only an explicit authenticated button connects a purchase; navigation and retries cannot mutate it. */
'use client';
import Link from 'next/link';
import { useRouter } from 'next/navigation';
import { AWS_MARKETPLACE_URL, PRIMARY, SECONDARY } from './constants';
import type { useMarketplace } from './use-marketplace';

/** The controller supplies the current account and narrowly scoped actions. */
type Props = ReturnType<typeof useMarketplace>;

/** Switching accounts preserves the sealed purchase context while clearing the Oya session. */
function OtherAccount({ auth }: Props) {
  const router = useRouter();
  return (
    <button
      className={SECONDARY}
      type="button"
      onClick={() => {
        auth.logout();
        router.push('/login');
      }}
    >
      Use another account
    </button>
  );
}

/** Account creation and sign-in use the same existing CAPTCHA/OAuth/MFA journey. */
function SignInActions() {
  return (
    <>
      <Link className={PRIMARY} href="/signup">
        Create account
      </Link>
      <Link className={SECONDARY} href="/login">
        Sign in
      </Link>
    </>
  );
}

/** Explicit account consent is required once and server-side retries preserve ownership. */
function ConnectActions(props: Props) {
  const { state, connect } = props;
  return (
    <>
      <button type="button" className={PRIMARY} disabled={state.busy} onClick={connect}>
        {state.busy ? 'Connecting…' : 'Connect to this account'}
      </button>
      <OtherAccount {...props} />
    </>
  );
}

/** Settled state actions do not offer checkout or claim paid activation. */
export function SetupActions(props: Props) {
  const { state, refresh } = props;
  if (state.kind === 'loading') return null;
  if (state.kind === 'signed-out') return <SignInActions />;
  if (state.kind === 'ready') return <ConnectActions {...props} />;
  if (state.kind === 'conflict') return <OtherAccount {...props} />;
  if (state.kind === 'changed')
    return (
      <button type="button" className={PRIMARY} disabled={state.busy} onClick={refresh}>
        Review purchase
      </button>
    );
  if (state.kind === 'pending' || state.kind === 'offline')
    return (
      <button type="button" className={PRIMARY} disabled={state.busy} onClick={refresh}>
        {state.busy ? 'Checking…' : state.kind === 'pending' ? 'Check again' : 'Try again'}
      </button>
    );
  return (
    <a className={PRIMARY} href={AWS_MARKETPLACE_URL}>
      Open AWS Marketplace
    </a>
  );
}
