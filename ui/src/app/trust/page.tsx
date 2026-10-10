/**
 * The trust center: where Oya stands on HIPAA, how the service is secured, the
 * controls the evidence pack proves, and who processes customer data. The control
 * table is read from compliance/evidence.json when the site is built, so the page
 * never claims a control the generated pack does not.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import type { Metadata } from 'next';
import styles from '../page.module.css';
import trust from './trust.module.css';
import { SiteHeader } from '../_home/site-header';
import { SiteFooter } from '../_home/site-footer';
import {
  EVIDENCE_PACK,
  PRACTICES,
  REPORT_VULNERABILITY,
  SALES_EMAIL,
  SELF_HOST_GUIDE,
  STATUS,
  SUBPROCESSORS,
} from './content';

/** Built once with the site: the evidence is read at build time, not per request. */
export const dynamic = 'force-static';

/** The page's title and description for search and link previews. */
export const metadata: Metadata = {
  title: 'Trust center',
  description: 'How Oya Browser secures customer data: HIPAA status, security practices, controls and subprocessors.',
  alternates: { canonical: '/trust' },
};

/** The generated evidence; the site is built from ui/, one folder down from compliance/. */
const EVIDENCE = path.join(process.cwd(), '..', 'compliance', 'evidence.json');

/** One HIPAA control as the evidence pack reports it. */
interface Control {
  /** Its section of the Security Rule. */
  id: string;
  /** Its name. */
  title: string;
  /** What the rule asks. */
  requirement: string;
  /** PASS, GAP or FAIL. */
  verdict: string;
}

/** The evidence pack as compliance/run-evidence.mjs writes it. */
interface Evidence {
  /** When the pack was generated, as an ISO timestamp. */
  generated: string;
  /** Every control, in the pack's order. */
  controls: Control[];
}

/** The evidence pack's controls and when it was generated. */
function readEvidence(): Evidence {
  return JSON.parse(readFileSync(EVIDENCE, 'utf8'));
}

/** Where each programme stands. */
function StatusCards() {
  return (
    <section className={trust.cards} aria-label="Compliance status">
      {STATUS.map((s) => (
        <article key={s.title} className={trust.card}>
          <span className={trust.state}>{s.state}</span>
          <h3>{s.title}</h3>
          <p>{s.body}</p>
        </article>
      ))}
    </section>
  );
}

/** How the service is secured. */
function Practices() {
  return (
    <section className={trust.section}>
      <h2>Security practices</h2>
      <div className={trust.grid}>
        {PRACTICES.map((p) => (
          <div key={p.title}>
            <h3>{p.title}</h3>
            <p>{p.body}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

/** The evidence pack's controls, gaps included. */
function Controls() {
  const { generated, controls } = readEvidence();
  return (
    <section className={trust.section}>
      <h2>HIPAA Security Rule controls</h2>
      <p className={trust.note}>
        Generated from the code on {generated.slice(0, 'YYYY-MM-DD'.length)}. Read the full{' '}
        <a href={EVIDENCE_PACK}>evidence pack</a>, with the check behind each verdict.
      </p>
      <table className={trust.table}>
        <thead>
          <tr>
            <th scope="col">Control</th>
            <th scope="col">Requirement</th>
            <th scope="col">Status</th>
          </tr>
        </thead>
        <tbody>
          {controls.map((c) => (
            <tr key={c.id}>
              <td>
                <strong>{c.title}</strong>
                <span className={trust.ref}>§{c.id}</span>
              </td>
              <td>{c.requirement}</td>
              <td className={c.verdict === 'PASS' ? trust.pass : trust.gap}>{c.verdict === 'PASS' ? 'Met' : 'Gap'}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

/** Who processes customer data for Oya Cloud. */
function Subprocessors() {
  return (
    <section className={trust.section}>
      <h2>Subprocessors</h2>
      <p className={trust.note}>
        Third parties that process customer data for Oya Cloud. Self-hosted deployments use none of them.
      </p>
      <table className={trust.table}>
        <thead>
          <tr>
            <th scope="col">Company</th>
            <th scope="col">Purpose</th>
            <th scope="col">Data</th>
          </tr>
        </thead>
        <tbody>
          {SUBPROCESSORS.map((s) => (
            <tr key={s.name}>
              <td>
                <strong>{s.name}</strong>
              </td>
              <td>{s.purpose}</td>
              <td>{s.data}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

/** Where to ask for a BAA or report a vulnerability. */
function Contact() {
  return (
    <section className={trust.section}>
      <h2>Contact</h2>
      <p>
        For a business associate agreement, a security questionnaire or our policies, write to{' '}
        <a href={`mailto:${SALES_EMAIL}`}>{SALES_EMAIL}</a>. To run Oya in your own cloud, read the{' '}
        <a href={SELF_HOST_GUIDE}>self-hosting guide</a>. To report a vulnerability,{' '}
        <a href={REPORT_VULNERABILITY}>open a private security advisory</a>.
      </p>
    </section>
  );
}

/** The trust center page. */
export default function TrustCenter() {
  return (
    <div className={styles.page}>
      <div className={styles.container}>
        <SiteHeader />
        <main className={trust.main}>
          <h1>Trust center</h1>
          <p className={trust.lead}>
            Oya runs browsers that sign in to the systems your business depends on. Here is how we protect what they
            see.
          </p>
          <StatusCards />
          <Practices />
          <Controls />
          <Subprocessors />
          <Contact />
        </main>
        <SiteFooter />
      </div>
    </div>
  );
}
