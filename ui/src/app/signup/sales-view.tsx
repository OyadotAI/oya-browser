/**
 * The card of sign-up's enterprise path: the questions, then the founders'
 * calendar from Calendly, which also shows the booking's confirmation.
 */
'use client';

import { bookingUrl } from './sales';
import { SalesForm } from './sales-form';
import { useSales, type Sales, type SalesStep } from './use-sales';

/** The progress labels, in step order. */
const STEPS: Array<[SalesStep, string]> = [
  ['details', 'Your details'],
  ['schedule', 'Pick a time'],
  ['booked', 'Confirmed'],
];

/** The card's heading and the line under it, per step. */
const HEADINGS: Record<SalesStep, [string, string]> = {
  details: ['Talk to sales', "Takes a minute. Next you'll pick a time."],
  schedule: ['Pick a time', '30 minutes, on video, with the Oya founders.'],
  booked: ["You're booked.", 'A calendar invite with the video link is on its way to your work email.'],
};

/** Progress' props. */
interface ProgressProps {
  /** The step showing. */
  step: SalesStep;
}

/** The numbered steps, the current one lit and earlier ones ticked. */
function Progress({ step }: ProgressProps) {
  const current = STEPS.findIndex(([s]) => s === step);
  return (
    <ol aria-label="Progress" className="mb-6 flex flex-wrap gap-x-4 gap-y-2 text-xs">
      {STEPS.map(([s, label], i) => (
        <li
          key={s}
          aria-current={i === current ? 'step' : undefined}
          className={i === current ? 'text-text' : 'text-text-dim'}
        >
          <span
            className={`mr-1.5 inline-flex h-5 w-5 items-center justify-center rounded-full text-xs font-bold ${i === current ? 'bg-accent text-accent-foreground' : 'border border-border'}`}
          >
            {i < current ? '✓' : i + 1}
          </span>
          {label}
        </li>
      ))}
    </ol>
  );
}

/** Schedule's props. */
interface ScheduleProps {
  /** The enterprise path's state. */
  sales: Sales;
}

/** The founders' calendar, prefilled, with a way back to the answers. */
function Schedule({ sales }: ScheduleProps) {
  const src = bookingUrl(sales.details, window.location.host);
  return (
    <div className="space-y-3">
      <iframe title="Pick a time with the Oya founders" src={src} className="h-[700px] w-full rounded-lg" />
      {sales.step === 'schedule' && (
        <button type="button" onClick={sales.edit} className="text-sm text-text-muted hover:text-text">
          ← Edit your details
        </button>
      )}
    </div>
  );
}

/** The enterprise path's card, from the first question to a booked call. */
export function SalesView() {
  const sales = useSales();
  const [heading, line] = HEADINGS[sales.step];
  return (
    <>
      <h1 className="mb-1 font-display text-2xl font-bold text-text">{heading}</h1>
      <p className="mb-6 text-sm text-text-muted">{line}</p>
      <Progress step={sales.step} />
      {sales.step === 'details' ? <SalesForm sales={sales} /> : <Schedule sales={sales} />}
      <p className="mt-4 text-xs text-text-dim">
        We only use this to prepare for the call. Rather email?{' '}
        <a href="mailto:sales@getoya.ai" className="text-accent hover:text-accent-hover">
          sales@getoya.ai
        </a>
      </p>
    </>
  );
}
