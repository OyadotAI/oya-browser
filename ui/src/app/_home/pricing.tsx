/** The pricing section: three priced plans and one priced by conversation, each with what it includes and one button. */
import Link from 'next/link';
import { Check } from 'lucide-react';
import { PLANS, PLAN_NOTE, type Plan } from './pricing-content';
import styles from './bold.module.css';

/** One plan card. */
function PlanCard({ plan }: { /** The plan shown. */ plan: Plan }) {
  return (
    <li className={styles.plan} data-lit={plan.lit ?? false}>
      <h3>{plan.name}</h3>
      <p className={styles.planLine}>{plan.line}</p>
      <p className={styles.planPrice}>
        {plan.price} <span>{plan.per}</span>
      </p>
      <ul>
        {plan.includes.map((item) => (
          <li key={item}>
            <Check size={14} aria-hidden="true" /> {item}
          </li>
        ))}
      </ul>
      <Link
        className={`btn ${plan.lit ? 'btn--primary' : 'btn--secondary'}`}
        href={plan.href}
        data-track="cta_clicked"
        data-track-label={`plan_${plan.name}`}
      >
        {plan.action}
      </Link>
    </li>
  );
}

/** The pricing section. */
export function Pricing() {
  return (
    <section className={`section ${styles.pricing}`} id="pricing" aria-labelledby="pricing-h">
      <div className="wrap">
        <h2 id="pricing-h">Start free. Pay as the work grows.</h2>
        <p className={styles.pricingLead}>
          Run your first task on your desktop at no cost, then pay for cloud hours when it is worth replaying.
        </p>
        <ul className={styles.plans}>
          {PLANS.map((plan) => (
            <PlanCard key={plan.name} plan={plan} />
          ))}
        </ul>
        <p className={styles.planNote}>{PLAN_NOTE}</p>
      </div>
    </section>
  );
}
