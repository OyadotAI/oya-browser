/**
 * Product analytics in the console: pageviews, sign-in and sign-up, and who the
 * signed-in person is. Off unless the server was started with POSTHOG_KEY and
 * POSTHOG_HOST, in which case the layout mounts it with those two values at
 * request time, so a self-host console never even loads the library. Nothing
 * is captured automatically: no clicked text, no session replay, because the
 * dashboard shows API keys and live browser frames.
 */
import type { PostHog } from 'posthog-js';

/** The client, once it has loaded. */
let client: PostHog | null = null;

/** Where analytics goes. */
type Settings = {
  /** The PostHog project's write key. */
  key: string;
  /** The PostHog host. */
  host: string;
};

/** How the library is configured: it captures nothing on its own, and keeps profiles only for people who signed in. */
const options = (host: string) => ({
  api_host: host,
  autocapture: false,
  capture_pageview: false,
  capture_pageleave: false,
  disable_session_recording: true,
  disable_external_dependency_loading: true,
  person_profiles: 'identified_only' as const,
});

/** Loads the library and points it at the operator's PostHog. Called once by the Analytics component. */
export async function init(settings: Settings) {
  if (client) return;
  const { default: posthog } = await import('posthog-js');
  posthog.init(settings.key, options(settings.host));
  client = posthog;
}

/** Records the page the person is on; a no-op until the library has loaded. */
export function pageview() {
  client?.capture('$pageview');
}

/** Records one product event with a few small properties. */
export function event(name: string, properties: Record<string, string | number | boolean> = {}) {
  client?.capture(name, properties);
}

/** What a person is described by. */
type PersonProperties = {
  /** Their sign-in email. */
  email?: string;
};

/** Says who the signed-in person is, so their events join up across visits. */
export function identify(id: string, properties: PersonProperties = {}) {
  client?.identify(id, properties);
}

/** Forgets who the person was, on sign-out, so the next person on this browser is not mistaken for them. */
export function reset() {
  client?.reset();
}
