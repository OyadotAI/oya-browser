/**
 * Docs: self-hosting. The one-line install, what the wizard lets you choose,
 * pointing the CLI at your deployment, and the settings that matter. The full
 * guide stays in the repository; this is the part a first install needs.
 */
'use client';

import type { ReactNode } from 'react';
import { CodeBlock, InlineCode, SectionHeading, Table, WarnBox } from '../_docs/blocks';

/** The self-hosting guide in the repository. */
const GUIDE = 'https://github.com/OyadotAI/oya-browser/blob/main/docs/self-hosting.md';

/** What the wizard lets you choose. */
const CHOICE_ROWS: ReactNode[][] = [
  ['Control plane', 'Docker on one machine; Amazon ECS, Kubernetes or Google Cloud for production'],
  ['Database', 'SQLite (default, one replica), Postgres (many replicas) or JSON files'],
  ['Browsers', 'Docker workers, one container or pod per session, Oya Cloud, or your own Chrome over CDP'],
  ['LLM', 'Anthropic, OpenAI, Gemini, any OpenAI-compatible endpoint, or a local model (Ollama, vLLM)'],
];

/** The settings a first deployment should know. */
const SETTING_ROWS: ReactNode[][] = [
  [
    <InlineCode key="a">OYA_PROFILE_SECRET</InlineCode>,
    'Encrypts cookies, passwords and TOTP seeds at rest. Generated into the data volume when unset, so keep that volume.',
  ],
  [<InlineCode key="b">API_KEYS</InlineCode>, 'Comma-separated tenant keys. The control plane needs nothing else.'],
  [<InlineCode key="c">OYA_STORAGE</InlineCode>, 'sqlite, postgres or file: where every table lives.'],
  [<InlineCode key="d">DATABASE_URL</InlineCode>, 'The Postgres connection string, with OYA_STORAGE=postgres.'],
  [<InlineCode key="e">OPENAI_API_KEY</InlineCode>, 'The default model for agents. Each API key can set its own.'],
];

/** Install, choose, connect. */
function Install() {
  return (
    <>
      <SectionHeading id="self-hosting">Self-hosting</SectionHeading>
      <p className="mb-3">
        Run the whole stack in your own network, for HIPAA and SOC 2 workloads or anywhere data may not leave. It needs
        git, Docker and Node 20 or later.
      </p>
      <CodeBlock label="Terminal">{`curl -fsSL https://raw.githubusercontent.com/OyadotAI/oya-browser/main/install.sh | sh`}</CodeBlock>
      <p className="mb-3">
        A wizard asks six questions, writes the config, starts the stack, waits until it is ready, and prints an API
        key. Add <InlineCode>--dry-run</InlineCode> after <InlineCode>sh -s --</InlineCode> to see the plan without
        writing anything.
      </p>
      <p className="mb-3">
        For agents and CI, <InlineCode>--yes</InlineCode> asks nothing: SQLite, the server and one browser worker in
        Docker on this machine. It ends by printing <InlineCode>OYA_BASE_URL</InlineCode> and{' '}
        <InlineCode>OYA_API_KEY</InlineCode>.
      </p>
      <CodeBlock label="Terminal">{`curl -fsSL https://raw.githubusercontent.com/OyadotAI/oya-browser/main/install.sh | sh -s -- --yes`}</CodeBlock>
      <Table headers={['Choice', 'Options']} rows={CHOICE_ROWS} />
      <p className="mb-3">
        To update to the latest version, run the update script. It keeps your settings and browser workers, rebuilds,
        restarts, and waits until the server is ready; with local edits in the checkout it stops and changes nothing.
      </p>
      <CodeBlock label="Terminal">{`curl -fsSL https://raw.githubusercontent.com/OyadotAI/oya-browser/main/update.sh | sh`}</CodeBlock>
    </>
  );
}

/** Pointing the CLI and SDK at the deployment, and the settings that matter. */
function Connect() {
  return (
    <>
      <h3 id="self-hosting-connect" className="text-[19px] font-semibold mt-10 mb-3 text-text">
        Point your code at it
      </h3>
      <CodeBlock label="Terminal">{`oya login --url http://localhost:3100 --key <the key it printed>
oya ls                             # the browsers that enrolled
oya goto https://example.com`}</CodeBlock>
      <p className="mb-3">
        In code, set <InlineCode>OYA_BASE_URL</InlineCode> next to <InlineCode>OYA_API_KEY</InlineCode>; everything else
        in these docs works unchanged.
      </p>
      <h3 id="self-hosting-settings" className="text-[19px] font-semibold mt-10 mb-3 text-text">
        Settings that matter
      </h3>
      <Table headers={['Variable', 'What it does']} rows={SETTING_ROWS} />
      <WarnBox>
        Keep the <InlineCode>oya-data</InlineCode> volume, or set <InlineCode>OYA_PROFILE_SECRET</InlineCode> yourself.
        Lose the secret and every stored login becomes unreadable.
      </WarnBox>
      <p className="mb-3">
        Databases, cloud browser runtimes, upgrades and every setting are in the{' '}
        <a href={GUIDE} className="text-accent hover:text-accent-hover transition-colors">
          full self-hosting guide
        </a>
        .
      </p>
    </>
  );
}

/** The self-hosting section. */
export function SelfHostingDocs() {
  return (
    <>
      <Install />
      <Connect />
    </>
  );
}
