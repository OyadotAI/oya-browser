/**
 * The public landing page. Route-scoped styles and controls keep its presentation isolated
 * from sign-in, documentation and the console. Each section lives in app/_home.
 */
import type { Metadata } from 'next';
import { headers } from 'next/headers';
import { Kit } from './_home/kit';
import { Nav } from './_home/nav';
import { Hero } from './_home/hero';
import { EnterpriseCapabilities } from './_home/enterprise-capabilities';
import { ProductDemo } from './_home/product-demo';
import { ControlPlaneOverview } from './_home/control-plane-overview';
import { Statement } from './_home/statement';
import { Integrations } from './_home/integrations';
import { Faq } from './_home/faq';
import { Closing } from './_home/closing';
import landing from './_home/landing.module.css';
import controlPlane from './_home/control-plane.module.css';

/** Classifies the public page without changing document-wide presentation state. */
export const metadata: Metadata = { other: { 'site-kind': 'software', 'page-type': 'software' } };

/** The landing page reads the request's nonce for its locally scoped font declaration. */
export default async function Home() {
  const nonce = (await headers()).get('x-nonce') ?? undefined;
  return (
    <div className={landing.page}>
      <Kit nonce={nonce} />
      <Nav />
      <main id="top" tabIndex={-1}>
        <Hero />
        <div className={landing.workflowSection}>
          <div className="wrap">
            <ProductDemo />
          </div>
        </div>
        <Statement />
        <section className="section" aria-labelledby="architecture-h">
          <div className="wrap">
            <div className={controlPlane.sectionHeading}>
              <h2 id="architecture-h">Operate those workflows across agents and browsers.</h2>
              <p>The control plane connects your applications, browser fleet and healthcare portals.</p>
            </div>
            <ControlPlaneOverview />
          </div>
        </section>
        <EnterpriseCapabilities />
        <Integrations />
        <Faq />
      </main>
      <Closing />
    </div>
  );
}
