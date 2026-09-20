/**
 * The project's webhook as the server returns it.
 */

/** The endpoint. */
export type Hook = {
  /** Endpoint id. */
  id: string;
  /** Where events are POSTed. */
  url: string;
  /** Event types sent; empty means all. */
  types: string[];
  /** Receiving events. */
  enabled: boolean;
};

/** One attempt to deliver an event. */
export type Delivery = {
  /** Delivery id, for replay. */
  id: string;
  /** delivered, pending or failed. */
  state: string;
  /** Tries so far. */
  attempts: number;
  /** When, in epoch milliseconds. */
  at: number;
  /** The event type, when known. */
  type: string | null;
};

/** GET /control/webhook. */
export type WebhookConfig = {
  /** The endpoint, or null when none was ever saved. */
  hook: Hook | null;
  /** Every event type there is. */
  events: string[];
  /** Recent deliveries. */
  deliveries: Delivery[];
};

/** PUT /control/webhook: the endpoint, with its secret only when one was just minted. */
export type SavedHook = Hook & {
  /** The new signing secret. */
  secret?: string;
};
