/** Deployment choices and runtime limits remain visible to enterprise evaluators. */
import Link from 'next/link';

/** These deployment choices describe existing hosting options, not different unverified guarantees. */
const DEPLOYMENTS = [
  ['Managed deployment', 'Run browsers on Oya Cloud and connect through the same control interfaces.'],
  ['Self-hosted deployment', 'Run the control plane and browser runtime in your own infrastructure.'],
];

/** Material limits help technical buyers evaluate the guarantees that apply to their deployment. */
const LIMITS = [
  'Strict host-policy enforcement requires the managed runtime. Other providers have different guarantees.',
  'Budget controls use configured rates and estimated spend; they are not provider invoice guarantees.',
  'Device-bound portal sessions may require sign-in again.',
  'Lifecycle webhooks use at-least-once delivery. Consumers must deduplicate events.',
];

/** Keeps the existing section export and anchor while adding explicit enterprise deployment detail. */
export function Integrations() {
  return (
    <section className="section deployment" id="deployment" aria-labelledby="deployment-h">
      <div className="wrap" id="integrations">
        <h2 id="deployment-h">Choose where those workflows run.</h2>
        <p className="deployment-lead">Use Oya Cloud or deploy in your own infrastructure.</p>
        <div className="deployment-columns">
          {DEPLOYMENTS.map(([title, body]) => (
            <div key={title}>
              <h3>{title}</h3>
              <p>{body}</p>
            </div>
          ))}
        </div>
        <div className="control-links">
          <Link className="link" href="/docs#self-hosting">
            Read deployment docs
          </Link>
          <Link className="link" href="/trust">
            Security and compliance
          </Link>
        </div>
        <details className="runtime-limits">
          <summary>Runtime limits</summary>
          <ul>
            {LIMITS.map((limit) => (
              <li key={limit}>{limit}</li>
            ))}
          </ul>
        </details>
      </div>
    </section>
  );
}
