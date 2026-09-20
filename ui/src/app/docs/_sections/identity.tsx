/**
 * Docs: identity: personas and rotation, CAPTCHA and MFA.
 */
'use client';

import type { ReactNode } from 'react';
import { CodeBlock, InlineCode, NoteBox, SectionHeading, Table, WarnBox } from '../_docs/blocks';

/** Rows of a table in the Personas section. */
const PERSONAS_ROWS_1: ReactNode[][] = [
  ['One account seen from many device fingerprints', 'Textbook bot farm'],
  ['One device fingerprint across many accounts — or 1,000 concurrent sessions', 'Device farm'],
];

/** The Personas section. */
function Personas() {
  return (
    <>
      {/* ============ ANONYMITY ============ */}
      {/* ============ PERSONAS ============ */}
      <SectionHeading id="personas">Personas</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        A <strong>persona</strong> is one identity: a fingerprint, a cookie jar and a proxy, bound together and stable
        for its life. One persona is one device.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        There are two ways to get caught, and they are mirror images of each other:
      </p>
      <Table headers={['Shape', 'Signal']} rows={PERSONAS_ROWS_1} />
      <p className="mb-3 text-[15px] leading-relaxed">
        Binding the fingerprint to your API key avoids the first and walks straight into the second. Binding it to each
        browser avoids the second and walks into the first. So the binding sits at the level that actually corresponds
        to a device:
      </p>
      <CodeBlock>{`persona = fingerprint + cookie jar + proxy       # one identity, one device
API key = a group of personas                    # your fleet`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        A persona&apos;s fingerprint is derived from a stored seed, so it is byte-identical across restarts — a
        returning session looks like a returning device, not a new one.
      </p>
      <CodeBlock>{`const p = await oya.personas.create({ name: "acme-ops" });
const browser = await oya.browser.start({ persona: p.id });

await oya.personas.list();     // includes activeBrowsers and maxConcurrent
await oya.personas.remove(p.id);`}</CodeBlock>
      <PersonasPart2 />
    </>
  );
}

/** The Personas section, continued. */
function PersonasPart2() {
  return (
    <>
      <NoteBox>
        Every API key has a <strong>default</strong> persona whose seed reproduces the fingerprint that key had before
        personas existed. If you run a single account, nothing changed for you.
      </NoteBox>

      <h3 id="rotation" className="text-base font-semibold mt-6 mb-2 text-text">
        Rotation and concurrency
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Rotation means picking a <em>different</em> persona — never giving one persona a new fingerprint.{' '}
        <InlineCode>persona: &apos;auto&apos;</InlineCode> selects the least recently used persona that is still under
        its concurrency cap.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Concurrency is capped per persona, because one laptop cannot be in a thousand places at once. Named personas
        default to 2 (a phone and a laptop is plausible); the default persona is uncapped so an existing fleet does not
        break on upgrade. Past the cap you get a clear 429 rather than a silent breach, and{' '}
        <InlineCode>activeBrowsers</InlineCode> is visible in the dashboard and as a Prometheus metric.
      </p>
    </>
  );
}

/** The CAPTCHA section. */
function Captcha() {
  return (
    <>
      {/* ============ CHALLENGES ============ */}
      <SectionHeading id="captcha">CAPTCHA</SectionHeading>
      <CodeBlock>{`await browser.solveCaptcha();                    // explicit
oya.browser.start({ captcha: 'auto' });          // solve as they appear`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        Detects reCAPTCHA v2/v3, hCaptcha and Turnstile. Providers that solve natively — Anchor, Browserbase, Steel,
        Browser Use — are left to do it rather than paying twice and racing their attempt. Everything else goes to your
        configured solver (CapSolver or 2Captcha).
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Returns <InlineCode>{`{ solved, method: 'provider' | 'solver' | 'none' }`}</InlineCode>. A failure returns{' '}
        <InlineCode>solved: false</InlineCode> — a silent no-op that leaves an agent stuck is worse than a clear answer.
      </p>
      <WarnBox>
        Automated solving conflicts with some sites&apos; terms of service. Sessions that used it are recorded in the
        audit trail so you can see which.
      </WarnBox>
    </>
  );
}

/** The MFA section. */
function Mfa() {
  return (
    <>
      <SectionHeading id="mfa">MFA</SectionHeading>
      <CodeBlock>{`await oya.personas.setMfa(id, { type: 'totp', secret: 'JBSWY3DPEHPK3PXP' });
await oya.personas.setMfa(id, { type: 'email', url: 'https://mail.example/api/latest' });

const r = await browser.completeMfa();
if (!r.completed) open(r.liveViewUrl);   // finish it by hand`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        TOTP is generated locally (RFC 6238). Email and SMS one-time codes are polled from a relay endpoint you supply,
        within a bounded window — the code does not exist yet when the prompt appears. When nothing automated can
        answer, <InlineCode>liveViewUrl</InlineCode> is where a person finishes; that is also the only workable answer
        for push-approval MFA.
      </p>
      <NoteBox>
        TOTP seeds are credential material of the same weight as a password: sealed at rest with AES-256-GCM, audited on
        use, and never returned by the API. The relay URL is checked against private and link-local ranges when you
        store it <em>and</em> on every poll, because a public name says nothing about where it resolves later.
      </NoteBox>
    </>
  );
}

/** Identity: personas and rotation, CAPTCHA and MFA. */
export function IdentityDocs() {
  return (
    <>
      <Personas />
      <Captcha />
      <Mfa />
    </>
  );
}
