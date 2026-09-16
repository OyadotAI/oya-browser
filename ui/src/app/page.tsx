import Image from 'next/image';
import Link from 'next/link';
import { ArrowRight, ArrowUpRight, Download, Braces, Activity, Network, ShieldCheck, Fingerprint, MousePointer2 } from 'lucide-react';
import { OyaWordmark } from '@/components/oya-logo';
import CopyExample from '@/components/copy-example';
import SyntaxCode from '@/components/ui/syntax-code';
import WorkflowDiagram from '@/components/workflow-diagram';
import styles from './page.module.css';
import { browserDownloads } from '@/lib/browser-downloads';

const repository = 'https://github.com/OyadotAI/oya-browser';
const portalExample = `import { Oya } from "@oya-ai/browser";

const oya = new Oya(); // Set OYA_API_KEY.
const { PORTAL_URL, PORTAL_USERNAME, PORTAL_PASSWORD } = process.env;
if (!PORTAL_URL || !PORTAL_USERNAME || !PORTAL_PASSWORD) {
  throw new Error("Set your test portal URL and credentials.");
}
const secrets = { username: PORTAL_USERNAME, password: PORTAL_PASSWORD };
const playbookName = "portal-request-review";

// First run: let the agent do the task, then store its playbook.
{
  await using browser = await oya.browser.start();
  await browser.goto(PORTAL_URL);
  await browser.ask(
    "If needed, log in with {{username}} and {{password}}. " +
    "Open New Request for {{customerName}}. Stop before submitting.",
    { data: { customerName: "Alex Example" }, secrets },
  );
  await browser.toPlaybook(playbookName);
} // The first browser stops here; the playbook stays saved.

// Later: run the saved playbook on a new browser with new inputs.
await using replay = await oya.browser.start();
await replay.goto(PORTAL_URL);
await replay.play(playbookName, {
  customerName: "Sam Example",
  ...secrets, // Replay remembers which variables are secret.
});`;


export default function Home() {
  return (
    <div className={styles.page}>
      <div className={styles.container}>
        <header className={styles.header}>
          <OyaWordmark />
          <nav aria-label="Main navigation" className={styles.navigation}>
            <Link href="/docs">Docs</Link>
            <a href={repository}>GitHub</a>
            <Link href="/dashboard" className={styles.console}>Open console</Link>
          </nav>
        </header>

        <main>
          <section className={styles.hero} aria-labelledby="hero-title">
            <div className={styles.heroCopy}>
              <p className={styles.eyebrow}>The browser for AI agents</p>
              <h1 id="hero-title">Run it once.<br /><span>Make it repeatable.</span></h1>
              <p className={styles.heroDescription}>Turn browser tasks into reusable playbooks. Replay the work, review repairs, and step in when your agent needs you.</p>
              <div className={styles.actions}>
                <Link href="/dashboard" className={styles.primary}>
                  Start building <ArrowUpRight size={17} aria-hidden="true" />
                </Link>
                <Link href="/docs" className={styles.secondary}>Read the docs <ArrowRight size={15} aria-hidden="true" /></Link>
              </div>
              <div id="download" className={styles.downloads}>
                <p>Download Oya Browser</p>
                <nav aria-label="Browser downloads">
                  {browserDownloads.map(({ platform, architecture, href }) => (
                    <a key={platform} href={href} download title={`${platform} · ${architecture}`}>
                      <Download size={17} aria-hidden="true" /> {platform}
                    </a>
                  ))}
                </nav>
                <Link href="/docs#download" className={styles.installHelp}>Installation instructions</Link>
              </div>
            </div>
            <WorkflowDiagram />
          </section>

          <section className={styles.providers} aria-label="Supported browser providers">
            <p>Your browser. Your choice.</p>
            <ul>
              {['Oya Cloud', 'Browserbase', 'Browser Use', 'Steel', 'Anchor', 'Your own Chrome'].map(provider => <li key={provider}>{provider}</li>)}
            </ul>
          </section>

          <section className={styles.controlPlane} aria-labelledby="control-plane-title">
            <div className={styles.controlPlaneIntro}>
              <div>
                <p className={styles.eyebrow}>The browser control plane</p>
                <h2 id="control-plane-title">Many browsers.<br /><span>One control plane.</span></h2>
              </div>
              <div>
                <p>Give your agents one place to connect. Choose where browsers run, see what they’re doing, and set the limits—all from Oya.</p>
                <Link href="/docs#control-plane">Explore the control plane <ArrowRight size={16} aria-hidden="true" /></Link>
              </div>
            </div>
            <figure className={styles.planeDiagram} aria-label="Your agents connect to the Oya control plane, which routes connections to cloud providers or your own Chrome.">
              <div className={styles.planeEndpoint}>
                <span className={styles.planeLabel}>YOUR AGENTS</span>
                <strong>The tools you use.</strong>
                <div className={styles.planeTags}><span>SDK</span><span>MCP</span><span>CDP</span><span>REST</span></div>
              </div>
              <div className={styles.planeConnector} aria-hidden="true"><ArrowRight size={18} /></div>
              <div className={styles.planeHub}>
                <Network size={25} strokeWidth={1.5} aria-hidden="true" />
                <strong>Oya</strong>
                <span>Route · Observe · Govern</span>
              </div>
              <div className={styles.planeConnector} aria-hidden="true"><ArrowRight size={18} /></div>
              <div className={styles.planeEndpoint}>
                <span className={styles.planeLabel}>YOUR BROWSERS</span>
                <strong>Wherever work runs.</strong>
                <div className={styles.planeProviders}><span>Oya Cloud</span><span>Your providers</span><span>Your own Chrome</span></div>
              </div>
            </figure>
            <div className={styles.controlPlaneBenefits}>
              <article>
                <Network size={19} strokeWidth={1.5} aria-hidden="true" />
                <h3>Choose where work runs.</h3>
                <p>Route new connections by priority, capacity, or latency. Change providers behind one shared endpoint.</p>
              </article>
              <article>
                <Activity size={19} strokeWidth={1.5} aria-hidden="true" />
                <h3>See what needs attention.</h3>
                <p>See fleet health and usage together. Trace what happened with audit history and session recordings.</p>
              </article>
              <article>
                <ShieldCheck size={19} strokeWidth={1.5} aria-hidden="true" />
                <h3>Scale with clear limits.</h3>
                <p>Set project access, budgets, and quotas. Keep resource use in view as your fleet grows.</p>
              </article>
            </div>
          </section>

          <section className={styles.control} aria-labelledby="control-title">
            <div className={styles.controlCopy}>
              <h2 id="control-title">Autonomous.<br /><span>Until you’re needed.</span></h2>
              <p>CAPTCHA, MFA, or a question. Open the live view, handle it, and let the run continue.</p>
              <Link href="/docs#dashboard-overview">See the console <ArrowUpRight size={16} aria-hidden="true" /></Link>
              <div className={styles.controlDetail}><MousePointer2 size={16} aria-hidden="true" /><span>Human handoff, built in.</span></div>
            </div>
            <figure className={styles.product}>
              <Image
                src="/oya-browser-poster.jpg"
                alt="Oya console showing three browsers and a live view with human takeover controls"
                width={1920}
                height={1080}
                sizes="(max-width: 960px) calc(100vw - 40px), 760px"
              />
              <figcaption>From the Oya console</figcaption>
            </figure>
          </section>

          <section id="developers" className={styles.developers} aria-labelledby="example-title">
            <div className={styles.exampleIntro}>
              <Braces size={24} strokeWidth={1.5} aria-hidden="true" />
              <h2 id="example-title">A task today.<br />A playbook tomorrow.</h2>
              <p>Your model. Your inputs. Playwright code you can inspect and keep.</p>
              <a href={`${repository}/tree/main/packages/sdk#portal-automation-record-once-replay-with-new-inputs`}>
                Get the full example <ArrowRight size={15} aria-hidden="true" />
              </a>
              <div className={styles.profileNote}><Fingerprint size={18} aria-hidden="true" /><span>Saved profiles keep your identity and login cookies across runs.</span></div>
            </div>
            <div className={styles.example}>
              <div className={styles.codeHeader}>
                <span>portal.ts <span className={styles.runtime}>Node 24+</span></span>
                <CopyExample code={portalExample} />
              </div>
              <pre tabIndex={0} aria-label="Portal automation TypeScript example"><SyntaxCode code={portalExample} language="typescript" /></pre>
              <p className={styles.codeNote}>Fictional inputs. Adapt the task to your test portal.</p>
            </div>
          </section>
        </main>

        <footer className={styles.footer}>
          <OyaWordmark />
          <nav aria-label="Footer navigation">
            <a href={`${repository}/tree/main/packages/sdk`}>SDK</a>
            <Link href="/docs#download">Desktop</Link>
            <a href={`${repository}/blob/main/docs/self-hosting.md`}>Self-host</a>
            <a href={repository}>GitHub</a>
          </nav>
        </footer>
      </div>
    </div>
  );
}
