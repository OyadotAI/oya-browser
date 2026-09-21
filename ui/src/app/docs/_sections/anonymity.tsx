/**
 * Docs: anonymity, fingerprints, proxies, stealth, and how a persona is chosen.
 *
 * There are no `list_profiles`, `create_profile` or `set_profile` tools, and there never
 * were: this page documented three of them for a while. A persona is made through the API
 * and chosen when the browser starts, which is what the sections below now say.
 */
'use client';

import { CodeBlock, InlineCode, NoteBox, SectionHeading } from '../_docs/blocks';

/** The Anonymity section. */
function Anonymity() {
  return (
    <>
      <SectionHeading id="anonymity">Anonymity</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Create and manage browser profiles with unique fingerprints, proxy routing, and isolated cookie stores. Each
        profile is a complete identity, different canvas hash, WebGL renderer, navigator properties, and session
        storage. Switch identities with a single MCP call.
      </p>
      <NoteBox>
        Every browser runs as a persona: a fingerprint, cookie jar and proxy bound together and stable for its life.
        Rotation means choosing a different persona, never re-rolling one.
      </NoteBox>

      <h3 id="fingerprint" className="text-base font-semibold mt-6 mb-2 text-text">
        Fingerprint Spoofing
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Each profile generates a coherent set of browser fingerprints that are internally consistent per platform. A
        Win32 profile gets Windows GPU strings, Windows fonts, and matching screen resolutions.
      </p>
      <AnonymityPart2 />
      <AnonymityPart3 />
      <AnonymityPart4 />
    </>
  );
}

/** The Anonymity section, continued. */
function AnonymityPart2() {
  return (
    <>
      <ul className="list-disc list-inside space-y-1 mb-4 text-[15px] leading-relaxed">
        <li>
          <strong>Canvas</strong>: deterministic pixel noise on <InlineCode>toDataURL</InlineCode> and{' '}
          <InlineCode>toBlob</InlineCode>
        </li>
        <li>
          <strong>WebGL</strong>: spoofed vendor/renderer strings from real GPU database
        </li>
        <li>
          <strong>AudioContext</strong>: noise on <InlineCode>OfflineAudioContext.startRendering</InlineCode>
        </li>
        <li>
          <strong>ClientRects</strong>: sub-pixel noise on <InlineCode>getBoundingClientRect</InlineCode> (bypassed
          internally for click accuracy)
        </li>
        <li>
          <strong>Navigator</strong>: platform, hardwareConcurrency, deviceMemory, languages, vendor
        </li>
        <li>
          <strong>Screen</strong>: width, height, colorDepth, devicePixelRatio
        </li>
        <li>
          <strong>WebRTC</strong>: ICE candidates stripped to prevent local IP leak
        </li>
        <li>
          <strong>Fonts</strong>: platform-consistent font sets
        </li>
      </ul>
    </>
  );
}

/** The Anonymity section, continued. */
function AnonymityPart3() {
  return (
    <>
      <h3 id="proxy-support" className="text-base font-semibold mt-6 mb-2 text-text">
        Proxy Support
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        A persona can take an HTTP, HTTPS or SOCKS5 proxy, given as one url. Chromium cannot authenticate to a SOCKS5
        proxy, so a proxy that needs a username and password must be HTTP or HTTPS. The proxy is applied at the session
        level, so all traffic routes through it, including DNS for SOCKS5. Timezone and locale are matched to the
        proxy&apos;s location over CDP, and a mismatch is reported rather than silently shipped.
      </p>
      <CodeBlock>
        {
          'const proxy = await oya.proxies.create({\n  url: "http://user:pass@1.2.3.4:8080",\n  geo: "us",\n  kind: "datacenter",\n});\n\nconst persona = await oya.personas.create({\n  name: "us-desktop",\n  prefs: { platform: "Win32", timezone: "America/New_York" },\n});\nawait oya.personas.pinProxy(persona.id, proxy.id);'
        }
      </CodeBlock>

      <h3 id="stealth" className="text-base font-semibold mt-6 mb-2 text-text">
        Anti-Detection Stealth
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Always active, no configuration needed. The stealth layer removes automation indicators that anti-bot systems
        check for:
      </p>
    </>
  );
}

/** The Anonymity section, continued. */
function AnonymityPart4() {
  return (
    <>
      <ul className="list-disc list-inside space-y-1 mb-4 text-[15px] leading-relaxed">
        <li>
          <InlineCode>navigator.webdriver</InlineCode> removed
        </li>
        <li>
          Electron globals (<InlineCode>window.process</InlineCode>, <InlineCode>window.require</InlineCode>) deleted
        </li>
        <li>
          <InlineCode>window.chrome</InlineCode> fixed to match real Chrome (app, runtime, csi, loadTimes)
        </li>
        <li>
          <InlineCode>navigator.plugins</InlineCode> populated with PDF viewers
        </li>
        <li>
          <InlineCode>navigator.permissions.query</InlineCode> patched
        </li>
        <li>Sec-CH-UA headers rewritten to hide Electron</li>
        <li>Google telemetry domains blocked at the network level</li>
      </ul>
    </>
  );
}

/** How a persona is made and chosen, which is the API rather than a tool call. */
function ChoosingAPersona() {
  return (
    <>
      <SectionHeading id="choosing-a-persona">Choosing a persona</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        A persona is made through the API and chosen when a browser starts. There is no tool that switches one
        mid-session: the device, cookie jar and proxy are bound together for the persona&apos;s life, so changing them
        would make the browser a different machine halfway through a run. To vary the device, clone the persona.
      </p>
      <CodeBlock>
        {
          'const personas = await oya.personas.list();\nconst browser = await oya.browser.start({ persona: personas[0].id });\n\n// A new device, fixed from here on:\nconst fresh = await oya.personas.create({ prefs: { platform: "MacIntel" } });\n\n// The same device, a second cookie jar:\nconst twin = await oya.personas.clone(fresh.id);'
        }
      </CodeBlock>
      <NoteBox>
        The console calls this workspace <strong>Profiles</strong>; the API and these docs call it a persona. Same
        thing.
      </NoteBox>
    </>
  );
}

/** Anonymity: fingerprints, proxies, stealth, and choosing a persona. */
export function AnonymityDocs() {
  return (
    <>
      <Anonymity />
      <ChoosingAPersona />
    </>
  );
}
