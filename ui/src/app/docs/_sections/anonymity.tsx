/**
 * Docs: anonymity: fingerprints, proxies, stealth, and the profile tools.
 */
'use client';

import type { ReactNode } from 'react';
import { CodeBlock, InlineCode, NoteBox, SectionHeading, Table, WarnBox } from '../_docs/blocks';

/** Rows of a table in the create_profile section. */
const CREATE_PROFILE_ROWS_1: ReactNode[][] = [
  [<InlineCode key="p">platform</InlineCode>, 'string', 'Win32, MacIntel, or Linux x86_64'],
  [<InlineCode key="tz">timezone</InlineCode>, 'string', 'IANA timezone (e.g. America/New_York)'],
  [<InlineCode key="lo">locale</InlineCode>, 'string', 'Locale (e.g. en-US, en-GB)'],
  [<InlineCode key="pt">proxy_type</InlineCode>, 'string', 'http or socks5'],
  [<InlineCode key="ph">proxy_host</InlineCode>, 'string', 'Proxy server hostname or IP'],
  [<InlineCode key="pp">proxy_port</InlineCode>, 'number', 'Proxy server port'],
  [<InlineCode key="pu">proxy_username</InlineCode>, 'string', 'Proxy auth username'],
  [<InlineCode key="pw">proxy_password</InlineCode>, 'string', 'Proxy auth password'],
];

/** Rows of a table in the set_profile section. */
const SET_PROFILE_ROWS_1: ReactNode[][] = [
  [<InlineCode key="pid">profile_id</InlineCode>, 'string (required)', 'ID of the profile to activate'],
];

/** The Anonymity section. */
function Anonymity() {
  return (
    <>
      <SectionHeading id="anonymity">Anonymity</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Create and manage browser profiles with unique fingerprints, proxy routing, and isolated cookie stores. Each
        profile is a complete identity — different canvas hash, WebGL renderer, navigator properties, and session
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
          <strong>Canvas</strong> — deterministic pixel noise on <InlineCode>toDataURL</InlineCode> and{' '}
          <InlineCode>toBlob</InlineCode>
        </li>
        <li>
          <strong>WebGL</strong> — spoofed vendor/renderer strings from real GPU database
        </li>
        <li>
          <strong>AudioContext</strong> — noise on <InlineCode>OfflineAudioContext.startRendering</InlineCode>
        </li>
        <li>
          <strong>ClientRects</strong> — sub-pixel noise on <InlineCode>getBoundingClientRect</InlineCode> (bypassed
          internally for click accuracy)
        </li>
        <li>
          <strong>Navigator</strong> — platform, hardwareConcurrency, deviceMemory, languages, vendor
        </li>
        <li>
          <strong>Screen</strong> — width, height, colorDepth, devicePixelRatio
        </li>
        <li>
          <strong>WebRTC</strong> — ICE candidates stripped to prevent local IP leak
        </li>
        <li>
          <strong>Fonts</strong> — platform-consistent font sets
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
        Each profile can include a SOCKS5 or HTTP/HTTPS proxy. The proxy is applied at the Electron session level — all
        traffic routes through it, including DNS (for SOCKS5). Timezone and locale auto-match the proxy&apos;s
        geographic location via CDP Emulation.
      </p>
      <CodeBlock>
        {
          'create_profile({\n  platform: "Win32",\n  timezone: "America/New_York",\n  proxy_type: "socks5",\n  proxy_host: "1.2.3.4",\n  proxy_port: 1080,\n  proxy_username: "user",\n  proxy_password: "pass"\n})'
        }
      </CodeBlock>

      <h3 id="stealth" className="text-base font-semibold mt-6 mb-2 text-text">
        Anti-Detection Stealth
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Always active — no configuration needed. The stealth layer removes automation indicators that anti-bot systems
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

/** The list_profiles section. */
function ListProfiles() {
  return (
    <>
      <SectionHeading id="list_profiles">list_profiles</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        List all available anonymity profiles on the connected browser. Shows which profile is active.
      </p>
      <CodeBlock>{'list_profiles()'}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        Returns each profile&apos;s ID, platform, timezone, and whether it has a proxy configured.
      </p>
    </>
  );
}

/** The create_profile section. */
function CreateProfile() {
  return (
    <>
      <SectionHeading id="create_profile">create_profile</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Create a new anonymity profile with a randomized browser fingerprint. All values are generated to be internally
        consistent for the chosen platform.
      </p>
      <CodeBlock>
        {'create_profile({\n  platform: "Win32",\n  timezone: "Europe/London",\n  locale: "en-GB"\n})'}
      </CodeBlock>
      <Table headers={['Param', 'Type', 'Description']} rows={CREATE_PROFILE_ROWS_1} />
    </>
  );
}

/** The set_profile section. */
function SetProfile() {
  return (
    <>
      <SectionHeading id="set_profile">set_profile</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Switch to a different anonymity profile. This closes all open tabs and reopens the browser with the new
        profile&apos;s fingerprint, proxy, timezone, and isolated cookie store.
      </p>
      <CodeBlock>{'set_profile({ profile_id: "profile-a1b2c3" })'}</CodeBlock>
      <Table headers={['Param', 'Type', 'Description']} rows={SET_PROFILE_ROWS_1} />
      <WarnBox>
        Switching profiles closes all open tabs. The browser reopens on google.com with the new identity.
      </WarnBox>
    </>
  );
}

/** Anonymity: fingerprints, proxies, stealth, and the profile tools. */
export function AnonymityDocs() {
  return (
    <>
      <Anonymity />
      <ListProfiles />
      <CreateProfile />
      <SetProfile />
    </>
  );
}
