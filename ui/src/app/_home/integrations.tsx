/** Deployment is a visual destination choice with complete runtime boundaries available on demand. */
import Link from 'next/link';
import { Cloud, Server } from 'lucide-react';
import { Reveal } from '@/components/ui/reveal';
import { Disclosure } from '@/components/ui/disclosure';

/** Existing descriptions remain complete in the optional comparison. */
const DEPLOYMENTS = [
  {
    title: 'Oya Cloud',
    short: 'Managed browsers.',
    full: 'Run browsers on Oya Cloud and connect through the same control interfaces.',
    icon: Cloud,
  },
  {
    title: 'Your infrastructure',
    short: 'Control plane and runtime.',
    full: 'Run the control plane and browser runtime in your own infrastructure.',
    icon: Server,
  },
];
/** Material limits apply to the existing runtime rather than a new marketing guarantee. */
const LIMITS = [
  'Strict host-policy enforcement requires the managed runtime. Other providers have different guarantees.',
  'Budget controls use configured rates and estimated spend; they are not provider invoice guarantees.',
  'Device-bound portal sessions may require sign-in again.',
  'Lifecycle webhooks use at-least-once delivery. Consumers must deduplicate events.',
];

/** The existing deployment anchors and evaluation links remain intact. */
export function Integrations() {
  return (
    <section className="section deployment" id="deployment" aria-labelledby="deployment-h">
      <div className="wrap" id="integrations">
        <Reveal>
          <h3 id="deployment-h">Choose where the work runs.</h3>
        </Reveal>
        <p className="deployment-lead">
          Use Oya Cloud or your own infrastructure. For work involving PHI, use the covered Oya Cloud or enterprise
          offering and request the BAA. Connected providers have different runtime guarantees; HIPAA mode uses Oya’s own
          browsers.
        </p>
        <div className="deployment-columns">
          {DEPLOYMENTS.map(({ title, short, icon: Icon }, index) => (
            <Reveal key={title} order={index}>
              <Icon size={36} aria-hidden="true" />
              <h4>{title}</h4>
              <p>{short}</p>
            </Reveal>
          ))}
        </div>
        <Disclosure summary="Compare deployment options">
          {DEPLOYMENTS.map(({ title, full }) => (
            <p key={title}>
              <strong>{title}.</strong> {full}
            </p>
          ))}
        </Disclosure>
        <div className="control-links">
          <Link className="link" href="/docs#self-hosting">
            Read deployment docs
          </Link>
          <Link className="link" href="/trust">
            Security and compliance
          </Link>
        </div>
        <Disclosure summary="Runtime limits">
          <ul>
            {LIMITS.map((limit) => (
              <li key={limit}>{limit}</li>
            ))}
          </ul>
        </Disclosure>
      </div>
    </section>
  );
}
