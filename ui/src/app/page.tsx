/**
 * The public landing page. Route-scoped styles and controls keep its presentation isolated
 * from sign-in, documentation and the console. Each section lives in app/_home.
 */
import type { Metadata } from 'next';
import { headers } from 'next/headers';
import { Kit } from './_home/kit';
import { Nav } from './_home/nav';
import { Hero } from './_home/hero';
import { ProductDemo } from './_home/product-demo';
import { Operations } from './_home/operations';
import { WorkflowStory } from './_home/workflow-story';
import { Statement } from './_home/statement';
import { BringYourStack } from './_home/bring-your-stack';
import { Faq } from './_home/faq';
import { Pricing } from './_home/pricing';
import { Closing } from './_home/closing';
import { LoudBand } from './_home/bold-band';
import landing from './_home/landing.module.css';
import bold from './_home/bold.module.css';

/** Classifies the public page without changing document-wide presentation state. */
export const metadata: Metadata = { other: { 'site-kind': 'software', 'page-type': 'software' } };

/** The landing page reads the request's nonce for its locally scoped font declaration. */
export default async function Home() {
  const nonce = (await headers()).get('x-nonce') ?? undefined;
  return (
    <div className={`${landing.page} ${bold.bold}`}>
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
        <LoudBand />
        <WorkflowStory />
        <BringYourStack />
        <Operations />
        <Pricing />
        <Faq />
      </main>
      <Closing />
    </div>
  );
}
