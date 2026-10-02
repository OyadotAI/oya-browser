/**
 * The sign-up page's body. Developers create an account (Google, GitHub or
 * email) and go on to the console; teams switch to Enterprise, answer a few
 * questions and book a call with the founders. The URL holds which:
 * `?plan=enterprise` is the second, so the sign-in page can link straight to it.
 */
'use client';

import Link from 'next/link';
import { useSearchParams } from 'next/navigation';
import { AuthLoading, AuthShell } from '@/components/auth/auth-shell';
import { ENTERPRISE_SIGNUP, EnterpriseNudge } from '@/components/auth/enterprise-nudge';
import { OAuthButtons } from '@/components/auth/oauth-buttons';
import { DEVELOPER_PITCH, Pitch, type PitchContent } from '@/components/auth/pitch';
import { ENTERPRISE_PITCH } from './sales';
import { SalesView } from './sales-view';
import { SignupForm } from './signup-form';
import { useSignup } from './use-signup';

/** Who is signing up. */
type Plan = 'developer' | 'enterprise';

/** The switch's choices, in order. */
const PLANS: Array<[Plan, string]> = [
  ['developer', 'Developer'],
  ['enterprise', 'Enterprise'],
];

/** The page's settings from the server. */
interface ViewProps {
  /** The Turnstile site key, '' when the captcha is off. */
  siteKey: string;
}

/** Where each plan lives; the URL holds the plan, so links and the back button land on the right one. */
const HREFS: Record<Plan, string> = { developer: '/signup', enterprise: ENTERPRISE_SIGNUP };

/** PlanSwitch's props. */
interface SwitchProps {
  /** The plan showing. */
  plan: Plan;
}

/** Developer or Enterprise, in the style of sign-in's mode switch. */
function PlanSwitch({ plan }: SwitchProps) {
  return (
    <nav
      aria-label="Account type"
      className="mb-8 grid w-full max-w-[380px] grid-cols-2 gap-1 rounded-lg border border-border p-1"
    >
      {PLANS.map(([p, label]) => (
        <Link
          key={p}
          href={HREFS[p]}
          scroll={false}
          aria-current={plan === p ? 'page' : undefined}
          className={`rounded-md px-3 py-1.5 text-center text-sm transition-colors ${plan === p ? 'bg-accent/10 text-text' : 'text-text-dim hover:text-text-muted'}`}
        >
          {label}
        </Link>
      ))}
    </nav>
  );
}

/** What each plan's page says beside its card. */
const PITCHES: Record<Plan, PitchContent> = { developer: DEVELOPER_PITCH, enterprise: ENTERPRISE_PITCH };

/** The plan in the URL's `?plan=`, Developer unless it says enterprise. */
function usePlan(): Plan {
  return useSearchParams()?.get('plan') === 'enterprise' ? 'enterprise' : 'developer';
}

/** The link back to sign-in, under either plan. */
const SIGN_IN = (
  <>
    Already have an account?{' '}
    <Link href="/login" className="font-medium text-accent hover:text-accent-hover">
      Sign in
    </Link>
  </>
);

/** Create an account, or book a sales call; `siteKey` turns the captcha on. */
export function SignupView({ siteKey }: ViewProps) {
  const { auth, form, fields, captcha, submit } = useSignup(siteKey);
  const plan = usePlan();
  // Don't render until auth state is resolved
  if (auth.loading || auth.user) return <AuthLoading />;
  return (
    <AuthShell
      glow="indigo"
      footer={SIGN_IN}
      above={<PlanSwitch plan={plan} />}
      pitch={<Pitch content={PITCHES[plan]} />}
    >
      {plan === 'enterprise' ? (
        <SalesView />
      ) : (
        <>
          <h1 className="mb-1 font-display text-2xl font-bold text-text">Create your account</h1>
          <p className="mb-8 text-sm text-text-muted">Get started with Oya Browser in seconds</p>
          <OAuthButtons />
          <SignupForm fields={fields} form={form} captcha={captcha} onSubmit={submit} />
          <EnterpriseNudge />
        </>
      )}
    </AuthShell>
  );
}
