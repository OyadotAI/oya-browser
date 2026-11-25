/** Visible pricing completes the workflow story with capacity choices and an enterprise conversation. */
import Link from 'next/link';
import { PUBLIC_PLANS, PAID_USAGE_NOTE, type PublicPlan } from './pricing-content';
import styles from './pricing.module.css';

/** Heading tag used for each plan card: h3 under the home page's h2, h2 under /pricing's h1. */
export type PlanHeading = 'h2' | 'h3';

/** A plan's action opens the existing destination rather than changing a subscription. */
function Plan({
  plan,
  headingAs = 'h3',
}: {
  /** Published allowance and destination. */ plan: PublicPlan;
  /** Heading tag matching the surrounding section outline. */ headingAs?: PlanHeading;
}) {
  const Heading = headingAs;
  return (
    <article className={styles.plan} aria-labelledby={`plan-${plan.name}`}>
      <Heading id={`plan-${plan.name}`}>{plan.name}</Heading>
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

/** The four published plans and their allowance, self-host and PHI notes, shared by / and /pricing. */
export function PricingPlans({
  planHeadingAs = 'h3',
}: {
  /** Heading tag for each plan column. */ planHeadingAs?: PlanHeading;
}) {
  return (
    <>
      <div className={styles.plans}>
        {PUBLIC_PLANS.map((plan) => (
          <Plan key={plan.name} plan={plan} headingAs={planHeadingAs} />
        ))}
      </div>
      <p className={styles.note}>Monthly cloud plans include the allowances shown. {PAID_USAGE_NOTE}</p>
      <p className={styles.note}>Self-hosting is free up to 5 cloud browsers at once, then $20 a browser a month.</p>
      <p className={styles.note}>
        Use synthetic data while developing on desktop. For protected health information (PHI), use Oya Cloud or the
        enterprise edition under a Business Associate Agreement (BAA).{' '}
        <Link className="link" href="/trust">
          Review security and compliance
        </Link>
      </p>
    </>
  );
}

/** All plan costs are readable without disclosure, with PHI scope next to the choice. */
export function Pricing() {
  return (
    <section className={`section ${styles.section}`} id="pricing" aria-labelledby="pricing-h">
      <div className="wrap">
        <h2 id="pricing-h">Start with the free desktop app.</h2>
        <p className={styles.lead}>Choose a cloud plan when you’re ready to run workflows there.</p>
        <PricingPlans planHeadingAs="h3" />
      </div>
    </section>
  );
}
