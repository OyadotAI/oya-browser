/**
 * The shapes the validation worker shares across its files: what the parent
 * sends to start a run, and the run's context every step reads and updates.
 */
import type { Page } from '@playwright/test';
import type { Draft } from '../workflow/index.ts';
import type { RunState } from './run-state.ts';

/** A message to the parent process. */
export type Tell = (message: Record<string, unknown>) => void;

/** A run event, before the run stamps and redacts it. */
export type RunEvent = Record<string, unknown>;

/** What the parent sends to start a run. */
export interface StartOptions {
  /** The normalized draft to replay. */
  draft: Draft;
  /** The run's variable values by name. */
  vars?: Record<string, unknown>;
  /** The step to pause at. */
  runTo?: string;
  /** Whether to report evidence notes per step. */
  evidence?: boolean;
  /** Whether a failing target may be swapped for a recorded alternative. */
  autoHeal?: boolean;
  /** The pause before each step, in milliseconds. */
  slowMo?: number;
  /** The run-scoped CDP front door to connect to. */
  endpoint: string;
  /** The run's token, sent to the front door. */
  token: string;
  /** Each run tab's name to the address that identifies it. */
  pageUrls?: Record<string, string>;
  /** Where the generated module is written. */
  directory: string;
  /** 'step' to pause after the first step. */
  command?: string;
}

/** Everything one run shares across its steps. */
export interface Run {
  /** The draft being replayed; repairs reorder its candidates. */
  draft: Draft;
  /** The run's variable values by name. */
  vars?: Record<string, unknown>;
  /** The step to pause at. */
  runTo?: string;
  /** Whether to report evidence notes per step. */
  evidence?: boolean;
  /** Whether a failing target may be swapped for a recorded alternative. */
  autoHeal: boolean;
  /** The pause before each step, in milliseconds. */
  slowMo: number;
  /** The run's controls. */
  state: RunState;
  /** Sends a message to the parent. */
  tell: Tell;
  /** Reports a run event, stamped and redacted. */
  emit: (event: RunEvent) => void;
  /** A message with the run's secrets removed. */
  redactMessage: (message: string) => string;
  /** The steps that passed. */
  done: Set<string>;
  /** Repairs tried per step. */
  attempts: Map<string, number>;
  /** When each step's repair window closes. */
  repairDeadlines: Map<string, number>;
  /** Each run tab by name. */
  pages: Map<string, Page>;
  /** Bumped per generated module, so each import is fresh. */
  revision: number;
  /** Whether the current step has sent input to the page. */
  inputIssued: boolean;
  /** When the current step started, in epoch milliseconds. */
  stepStarted: number;
  /** The step being repaired, so its failure reruns rather than ends the run. */
  repairSignal: string | null;
}
