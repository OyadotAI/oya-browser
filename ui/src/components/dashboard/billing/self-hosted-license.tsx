/**
 * The billing page's self-hosted license section: buy a license for more than
 * the 5 cloud browsers a self-hosted server runs free, then copy its key. A
 * self-hoster's 402 links here (#self-hosted). Its state lives in
 * `use-self-hosted-license.ts`.
 */
'use client';

import { Loader2 } from 'lucide-react';
import { dateOf } from './model';
import { FOUNDERS_CALL } from './constants';
import { LICENSE_ANCHOR, LICENSE_MAX, LICENSE_MIN, licenseState, quantityProblem, totalOf } from './license-model';
import type { License, LicenseInterval } from './license-model';
import { useSelfHostedLicense, type SelfHostedLicenseState } from './use-self-hosted-license';

/** A plain secondary button, the page's look for a way out to Stripe. */
const Button = (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
  <button
    {...props}
    className="self-start rounded-md border border-border px-3 py-1.5 text-xs font-medium text-text hover:bg-text/5 disabled:opacity-50"
  />
);

/** The founders' booking page, as a link. */
const Founders = () => (
  <a className="underline" href={FOUNDERS_CALL} target="_blank" rel="noreferrer">
    book a time with the founders
  </a>
);

/** Why the count asked for cannot be bought, beside its field. */
function QuantityError({ problem }: { /** quantityProblem's answer. */ problem: string }) {
  if (!problem) return null;
  return (
    <p id="license-browsers-error" className="text-xs text-red">
      {problem === 'over' ? (
        <>
          For more than {LICENSE_MAX}, <Founders />
        </>
      ) : (
        problem
      )}
    </p>
  );
}

/** The billing period choice. */
function Period({ form }: Pick<SelfHostedLicenseState, 'form'>) {
  const choices: [LicenseInterval, string][] = [
    ['month', 'Monthly, $20 per browser'],
    ['year', 'Yearly, $240 per browser'],
  ];
  return (
    <fieldset className="flex flex-col gap-1">
      <legend className="text-xs text-text-muted">Billing</legend>
      {choices.map(([value, label]) => (
        <label key={value} className="flex items-center gap-2 text-sm text-text">
          <input
            type="radio"
            name="license-interval"
            checked={form.interval === value}
            onChange={() => form.setInterval(value)}
          />
          {label}
        </label>
      ))}
    </fieldset>
  );
}

/** The form to buy a license: how many browsers, how often billed, the total. */
function BuyForm({ s }: { /** The section's state. */ s: SelfHostedLicenseState }) {
  const { browsers, setBrowsers, interval } = s.form;
  const problem = quantityProblem(browsers);
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-text-secondary">
        Run more than 5 cloud browsers on your own server. $20 per browser per month.
      </p>
      <label className="flex flex-col gap-1 text-xs text-text-muted">
        Concurrent browsers
        <input
          type="number"
          min={LICENSE_MIN}
          max={LICENSE_MAX}
          value={Number.isNaN(browsers) ? '' : browsers}
          onChange={(e) => setBrowsers(e.target.valueAsNumber)}
          aria-invalid={Boolean(problem)}
          aria-describedby={problem ? 'license-browsers-error' : undefined}
          className="w-28 rounded-md border border-border-control bg-bg-input px-2 py-1 text-sm text-text"
        />
      </label>
      <QuantityError problem={problem} />
      <Period form={s.form} />
      {!problem && <p className="text-sm text-text">{totalOf(browsers, interval)}</p>}
      <Button onClick={() => void s.buy()} disabled={Boolean(problem) || s.busy}>
        {s.busy ? 'Opening checkout…' : 'Buy license'}
      </Button>
      {s.error && <p className="text-xs text-red">Could not open checkout. Try again.</p>}
    </div>
  );
}

/** The key, read-only, with a button that copies it. */
function KeyField({
  s,
  license,
}: {
  /** The section's state. */ s: SelfHostedLicenseState;
  /** The license. */ license: License;
}) {
  return (
    <div className="flex flex-col gap-2 sm:flex-row">
      <input
        readOnly
        aria-label="License key"
        value={license.key}
        onFocus={(e) => e.target.select()}
        className="min-w-0 flex-1 rounded-md border border-border bg-bg-sunken px-2 py-1 font-mono text-xs text-text"
      />
      <Button onClick={s.copy}>Copy</Button>
      <span aria-live="polite" className="self-center text-xs text-text-dim">
        {s.copied ? 'Copied' : ''}
      </span>
    </div>
  );
}

/** One line on the license's billing: renewing, failed payment, or ending. */
function Standing({ license }: { /** The license. */ license: License }) {
  const until = dateOf(license.expiresAt);
  const state = licenseState(license);
  if (state === 'past_due')
    return (
      <p className="text-xs text-yellow">
        Your last payment failed. No new key is issued until it succeeds. Your current key works until {until}.
      </p>
    );
  if (state === 'expired')
    return (
      <p className="text-xs text-text-secondary">
        Your key stopped working {until}. 5 browsers run free until you buy a new license.
      </p>
    );
  if (state === 'cancelled')
    return (
      <p className="text-xs text-text-secondary">Cancelled. Your key works until {until}, then 5 browsers run free.</p>
    );
  return null;
}

/** The person's license: its size, renewal, key and how to use it. */
function ActiveLicense({
  s,
  license,
}: {
  /** The section's state. */ s: SelfHostedLicenseState;
  /** The license. */ license: License;
}) {
  const period = license.interval === 'year' ? 'yearly' : 'monthly';
  const ended = ['cancelled', 'expired'].includes(licenseState(license));
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-text">
        {license.browsers} browsers, {period}.{license.renewsAt ? ` Renews ${dateOf(license.renewsAt)}.` : ''}
      </p>
      {license.previousExpiresAt && (
        <p className="text-xs text-yellow">
          New key issued {dateOf(license.issuedAt)}. Replace <code>OYA_LICENSE_KEY</code> before{' '}
          {dateOf(license.previousExpiresAt)}, when the old key stops.
        </p>
      )}
      <Standing license={license} />
      <KeyField s={s} license={license} />
      <p className="text-xs text-text-secondary">
        Set it as <code>OYA_LICENSE_KEY</code> on your server and restart. This key works until{' '}
        {dateOf(license.expiresAt)}.
      </p>
      <Button onClick={() => void s.manage()}>
        {licenseState(license) === 'past_due' ? 'Update card' : 'Change card, see receipts or cancel'}
      </Button>
      {ended && <BuyForm s={s} />}
    </div>
  );
}

/** The wait for the key after paying. */
function Pending({ slow }: { /** Whether it is taking long. */ slow: boolean }) {
  if (slow)
    return (
      <div className="flex flex-col gap-2 text-sm text-text-secondary">
        <p>
          Still issuing. Refresh in a minute, or <Founders />.
        </p>
        <Button onClick={() => window.location.reload()}>Refresh</Button>
      </div>
    );
  return (
    <p className="flex items-center gap-2 text-sm text-text-secondary" role="status">
      <Loader2 className="h-4 w-4 animate-spin" /> Payment received. Issuing your key…
    </p>
  );
}

/** What the section shows for the person's state. */
function Body({ s }: { /** The section's state. */ s: SelfHostedLicenseState }) {
  if (s.pending) return <Pending slow={s.slow} />;
  if (s.data?.license) return <ActiveLicense s={s} license={s.data.license} />;
  return <BuyForm s={s} />;
}

/** The self-hosted license section; nothing on a self-hosted server or before it loads. */
export function SelfHostedLicense() {
  const s = useSelfHostedLicense();
  if (!s.data?.enabled) return null;
  return (
    <section id={LICENSE_ANCHOR} className="flex flex-col gap-3">
      <div className="border-b border-border pb-2">
        <h2 id={`${LICENSE_ANCHOR}-title`} tabIndex={-1} className="text-sm font-semibold tracking-tight text-text">
          Self-hosted license
        </h2>
      </div>
      <Body s={s} />
    </section>
  );
}
