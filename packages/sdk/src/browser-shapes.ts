/**
 * The shapes Browser's methods take and return that are not part of the
 * exported types: tab listings, dialog answers, share links and the like.
 * Named here so each field can say what it is.
 */
import type { RunData } from './types/index.js';

/** What a browser command answers. */
export interface CommandResult<T = unknown> {
  /** False when the command ran and failed. */
  ok: boolean;
  /** The command's output. */
  data?: T;
  /** Why it failed. */
  error?: string;
}

/** What `type()` reports back. */
export interface TypeResult {
  /** An autocomplete list opened under the field; pick from it before moving on. */
  suggestions_visible?: boolean;
}

/** The dialog `handleDialog()` answered. */
export interface DialogResult {
  /** alert, confirm, prompt or beforeunload. */
  type: string;
  /** The dialog's text. */
  message: string;
  /** Whether it was accepted. */
  accepted: boolean;
}

/** A point on the page, in CSS pixels. */
export interface Point {
  /** From the left edge. */
  x: number;
  /** From the top edge. */
  y: number;
}

/** One open tab. */
export interface Tab {
  /** The tab's id, for `switchTab()` and `closeTab()`. */
  id: string;
  /** The page it shows. */
  url: string;
  /** The page's title. */
  title: string;
  /** Whether it is the tab commands act on. */
  active: boolean;
}

/** Task values for `ask()`. */
export interface AskValues {
  /** Values the agent can read. */
  data?: RunData;
  /** Values the agent never sees. */
  secrets?: RunData;
}

/** How `play()` handles a step that no longer fits. */
export interface PlayOptions {
  /** Let the agent finish the task and save its fix as a draft. Default true. */
  autoHeal?: boolean;
}

/** What `submit()` runs: a prompt for the agent, or a saved playbook. */
export type Task =
  | {
      /** A natural-language task for the agent. */
      prompt: string;
    }
  | {
      /** The name of a saved playbook. */
      playbook: string;
    };

/** How a `shareUrl()` link works. */
export interface ShareOptions {
  /** Let whoever opens it act in the browser. Default view-only. */
  control?: boolean;
  /** How long it lasts. Default one hour. */
  expiresInSeconds?: number;
}

/** A link from `shareUrl()`. */
export interface ShareLink {
  /** The link to hand out. */
  url: string;
  /** Its credential's id, for `revokeShare()`. */
  id: string;
  /** When it stops working, in epoch milliseconds; null for never. */
  expiresAt: number | null;
}

/** The `screenshot` command's output. */
export interface ScreenshotData {
  /** A `data:image/…;base64,` URL. */
  screenshot: string;
}

/** The `list_tabs` command's output. */
export interface TabsData {
  /** Every open tab. */
  tabs: Array<Tab>;
}

/** The `open_tab` command's output. */
export interface OpenTabData {
  /** The new tab's id. */
  tab_id: string;
}

/** A live-stream connection ticket. */
export interface TicketData {
  /** Single use, short-lived. */
  ticket: string;
}

/** The chat endpoint's answer. */
export interface AgentText {
  /** The agent's reply. */
  text: string;
}
