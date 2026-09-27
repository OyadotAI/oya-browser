/**
 * Docs: playbooks. A task done once by the agent, saved as Playwright steps,
 * and replayed with no model: variables and secrets, free-text fields, healing,
 * signing in on the way, the generated code, moving between environments and
 * background runs.
 */
'use client';

import type { ReactNode } from 'react';
import { CodeBlock, InlineAnchor, InlineCode, NoteBox, SectionHeading, Table } from '../_docs/blocks';
import { useDocsNav } from '../_docs/nav';

/** A sub-heading's props. */
interface SubProps {
  /** The anchor the sidebar and search jump to. */
  id: string;
  /** The heading's text. */
  children: ReactNode;
}

/** An h3 inside the playbooks section, with the anchor the sidebar jumps to. */
function Sub({ id, children }: SubProps) {
  return (
    <h3 id={id} className="text-[19px] font-semibold mt-10 mb-3 text-text">
      {children}
    </h3>
  );
}

/** The playbook endpoints, for callers without the SDK. */
const REST_ROWS: ReactNode[][] = [
  ['POST', <InlineCode key="a">/api/browsers/:id/playbooks</InlineCode>, 'Save the browser’s last run: { name }'],
  ['POST', <InlineCode key="b">/api/browsers/:id/playbooks/:name/play</InlineCode>, 'Replay: { variables, autoHeal }'],
  ['GET', <InlineCode key="c">/api/playbooks</InlineCode>, 'Every saved playbook'],
  ['GET', <InlineCode key="d">/api/playbooks/:name/export</InlineCode>, 'One playbook as a JSON document'],
  ['POST', <InlineCode key="e">/api/playbooks/import</InlineCode>, 'Save an export: { playbook, name?, overwrite? }'],
  ['DELETE', <InlineCode key="f">/api/playbooks/:name</InlineCode>, 'Delete a playbook'],
];

/** What a playbook is, and the record-then-replay loop. */
function Overview() {
  return (
    <>
      <SectionHeading id="playbooks">Playbooks</SectionHeading>
      <p className="mb-3">
        A playbook is a task the agent did once, kept as Playwright steps. Replaying it runs those steps with no model
        in the loop, so it is fast, costs no tokens and does the same thing every time.
      </p>
      <CodeBlock label="TypeScript">{`await browser.ask("Check eligibility for member {{memberId}}", {
  data: { memberId: "W2847-1193" },
});
const playbook = await browser.toPlaybook("eligibility-check");

// Later, on every case:
const result = await browser.play("eligibility-check", { memberId: "W5512-0042" });
// { steps: 7, total: 7, fellBack: false }`}</CodeBlock>
      <p className="mb-3">
        In the console the same loop is a button: run a prompt, then press <strong>Save as playbook</strong>.
      </p>
    </>
  );
}

/** Variables, defaults and secrets. */
function Variables() {
  return (
    <>
      <Sub id="playbook-variables">Variables and secrets</Sub>
      <p className="mb-3">
        Put values in the prompt as <InlineCode>{'{{name}}'}</InlineCode>. Anything in <InlineCode>data</InlineCode> the
        agent can read; anything in <InlineCode>secrets</InlineCode> it never sees. Either way it types them through the
        placeholder, so the saved playbook keeps names, not values.
      </p>
      <CodeBlock label="TypeScript">{`await browser.ask("Sign in as {{user}} with {{password}}, then open claims", {
  data: { user: "ops@clinic.example" },
  secrets: { password: process.env.PORTAL_PASSWORD },
});`}</CodeBlock>
      <p className="mb-3">
        <InlineCode>playbook.variables</InlineCode> lists the inputs and <InlineCode>playbook.defaults</InlineCode> what
        the run used, so <InlineCode>play(name)</InlineCode> with nothing repeats it and you pass only what changes. A
        secret has no default: it never left the page.
      </p>
    </>
  );
}

/** Free-text fields the model writes fresh each run. */
function FreeText() {
  return (
    <>
      <Sub id="free-text-fields">Free-text fields</Sub>
      <p className="mb-3">
        A comment box or a question to answer should not be the same text every run. Oya spots these fields and leaves
        them to your model, one short call per field, listed in <InlineCode>playbook.answers</InlineCode>. The generated
        code reads:
      </p>
      <CodeBlock label="eligibility-check.js">{`await page.getByLabel("Reason for visit").fill(
  vars["reason"] ?? (await oya.llm.answer("Reason for visit", vars)),
);`}</CodeBlock>
      <p className="mb-3">
        Pass the field&apos;s key in <InlineCode>play()</InlineCode> to type fixed text instead. A playbook without such
        a field makes no model call at all.
      </p>
    </>
  );
}

/** Healing, and signing in on the way. */
function Healing() {
  const { navigate } = useDocsNav();
  return (
    <>
      <Sub id="healing">When a page changes</Sub>
      <p className="mb-3">
        If a step no longer fits the page, the agent finishes the task from there and its fix replaces the broken steps,
        so the next replay runs clean. The result says <InlineCode>healed: true</InlineCode>. Pass{' '}
        <InlineCode>{'{ autoHeal: false }'}</InlineCode> to throw the step&apos;s error instead.
      </p>
      <Sub id="replay-sign-in">Signing in on replay</Sub>
      <p className="mb-3">
        A replay that lands on a login page signs in with the persona&apos;s stored login and second factor, then
        carries on. Store them once; they are sealed at rest and never read back. See{' '}
        <InlineAnchor onClick={() => navigate('mfa')}>MFA</InlineAnchor> for the code types.
      </p>
      <CodeBlock label="TypeScript">{`await oya.personas.setCredentials(personaId, {
  domain: "portal.payer.example",
  username: "ops@clinic.example",
  password: process.env.PORTAL_PASSWORD,
});
await oya.personas.setMfa(personaId, { type: "totp", secret: process.env.PORTAL_TOTP });`}</CodeBlock>
    </>
  );
}

/** The generated Playwright module. */
function PlaywrightCode() {
  return (
    <>
      <Sub id="playwright-code">The Playwright code</Sub>
      <p className="mb-3">
        <InlineCode>playbook.code</InlineCode> is the same flow as a Playwright module you own. Read it in review, keep
        it in git, or run it on any Playwright page:
      </p>
      <CodeBlock label="TypeScript">{`// playbook.code, saved as eligibility-check.js:
// export default async function run(page, vars, oya) { ... }
import run from "./eligibility-check.js";

await run(page, { memberId: "W5512-0042" }, oya);`}</CodeBlock>
    </>
  );
}

/** Moving playbooks between environments. */
function ExportImport() {
  return (
    <>
      <Sub id="export-import">Export and import</Sub>
      <p className="mb-3">
        Move a playbook from staging to production, or from the cloud to your own deployment. Secrets travel by name
        only; set their values where it lands.
      </p>
      <CodeBlock label="TypeScript">{`const doc = await staging.playbooks.export("eligibility-check");
await production.playbooks.import(doc, { overwrite: true });`}</CodeBlock>
      <CodeBlock label="Terminal">{`oya playbooks export eligibility-check --out eligibility-check.json
oya playbooks import eligibility-check.json --replace`}</CodeBlock>
    </>
  );
}

/** Background runs with callbacks, and the REST endpoints. */
function BackgroundRuns() {
  return (
    <>
      <Sub id="background-runs">Background runs</Sub>
      <p className="mb-3">
        For long queues, submit a replay and hear back. <InlineCode>onHumanAttention</InlineCode> fires for a CAPTCHA or
        code nobody could solve; the run waits until you <InlineCode>respond()</InlineCode>.
      </p>
      <CodeBlock label="TypeScript">{`await browser.submit({ playbook: "eligibility-check" }, {
  data: { memberId: "W5512-0042" },
  onSuccess: (result) => save(result),
  onHealed: (result) => notify("the portal changed; the playbook was fixed"),
  onHumanAttention: async (request) => { /* finish by hand */ await request.respond("done"); },
});`}</CodeBlock>
      <Table headers={['Method', 'Endpoint', 'What it does']} rows={REST_ROWS} />
      <NoteBox>Every endpoint takes the API key as a bearer token, like the rest of the REST API.</NoteBox>
    </>
  );
}

/** The playbooks section. */
export function PlaybooksDocs() {
  return (
    <>
      <Overview />
      <Variables />
      <FreeText />
      <Healing />
      <PlaywrightCode />
      <ExportImport />
      <BackgroundRuns />
    </>
  );
}
