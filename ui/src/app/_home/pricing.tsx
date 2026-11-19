/** Visible pricing completes the workflow story with capacity choices and an enterprise conversation. */
import Link from 'next/link';
import { PUBLIC_PLANS, PAID_USAGE_NOTE, type PublicPlan } from './pricing-content';
import styles from './pricing.module.css';

/** A plan's action opens the existing destination rather than changing a subscription. */
function Plan({ plan }: { /** Published allowance and destination. */ plan: PublicPlan }) {
  return (
    <article className={styles.plan} aria-labelledby={`plan-${plan.name}`}>
      <h3 id={`plan-${plan.name}`}>{plan.name}</h3>
      <p className={styles.description}>{plan.description}</p>
      <p className={styles.price}>
        {plan.price} <span>{plan.period}</span>
      </p>
      <ul aria-label={`${plan.name} includes`}>
        {plan.includes.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
      <Link className="btn btn--secondary" href={plan.href}>
        {plan.action}
      </Link>
    </article>
  );
}

/** All plan costs are readable without disclosure, with PHI scope next to the choice. */
export function Pricing() {
  return (
    <section className={`section ${styles.section}`} id="pricing" aria-labelledby="pricing-h">
      <div className="wrap">
        <h2 id="pricing-h">Start with one workflow. Choose capacity as it grows.</h2>
        <p className={styles.lead}>
          The desktop app is free. Choose a cloud plan when you’re ready to run workflows there.
        </p>
        <div className={styles.plans}>
          {PUBLIC_PLANS.map((plan) => (
            <Plan key={plan.name} plan={plan} />
          ))}
        </div>
        <p className={styles.note}>Monthly cloud plans include the allowances shown. {PAID_USAGE_NOTE}</p>
        <p className={styles.note}>Self-hosting is free up to 5 cloud browsers at once.</p>
        <p className={styles.note}>
          Use synthetic data while developing on desktop. For protected health information (PHI), use Oya Cloud or the
          enterprise edition under a Business Associate Agreement (BAA).{' '}
          <Link className="link" href="/trust">
            Review security and compliance
          </Link>
        </p>
      </div>
    </section>
  );
}
