/**
 * oya install — stand up a self-hosted control plane.
 *
 * `oya login` + `oya init` configure a control plane that already exists, against
 * one API key. This command creates the control plane itself: the database, the
 * server, the browser fleet, the LLM and the optional services — then hands off
 * to those two, so provider questions are asked once, by the code that owns them.
 *
 * Everything it decides is written to oya-install.json (no secrets), so a second
 * run, a CI run or a colleague reproduces the same deployment with --config.
 *
 * This file is the facade and runs the stages in order; each stage is in install/.
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { banner, note, steps, success, warn } from './prompt.ts';
import { locateRepo } from './install/repo.ts';
import { interview } from './install/interview.ts';
import { preflight } from './install/preflight.ts';
import { readEnv, renderEnv, writeEnv } from './install/env-file.ts';
import { buildEnv, type BuiltEnv } from './install/build-env.ts';
import { priorState, resolveKekConflict } from './install/kek.ts';
import { migrate } from './install/migrate.ts';
import { provisionK8sFleet } from './install/k8s.ts';
import { provisionDocker, waitReady } from './install/docker.ts';
import { printDone, printDryRun, printPlan, printPreflight } from './install/report.ts';
import type { Answers, Interview } from './install/types.ts';
import { INTERVIEW_STEPS, JSON_INDENT } from './install/constants.ts';

export type { Answers } from './install/types.ts';

/** Where the answers are saved, beside .env. */
const ANSWERS_FILE = 'oya-install.json';

/** A resolved install: where it runs, what was decided, and what .env will hold. */
interface Plan extends Interview {
  /** The checkout. */
  root: string;
  /** Its .env. */
  envPath: string;
  /** What .env held before, if anything. */
  existing: Record<string, string>;
  /** The values to write. */
  env: BuiltEnv;
}

/** The interview's result, and the checkout it is for. */
interface Gathered extends Interview {
  /** The checkout. */
  root: string;
}

/** Finds the checkout, loads any --config plan, and asks the rest. */
async function gather(configPath: string | null): Promise<Gathered> {
  const root = await locateRepo();
  const preset: Partial<Answers> = configPath ? JSON.parse(readFileSync(configPath, 'utf8')) : {};
  banner('Oya Browser — self-host install', root);
  if (configPath) note(`replaying ${configPath}`);
  // A replay asks nothing, so numbering the handful of surviving prompts would
  // count to six and never get there.
  steps(configPath ? 0 : INTERVIEW_STEPS);
  return { root, ...(await interview(preset, !!configPath)) };
}

/** Prints preflight; a preview reports what is missing, only a real install refuses to proceed. */
async function checkMachine(root: string, answers: Answers, dryRun: boolean): Promise<void> {
  const checks = await preflight(root, answers);
  printPreflight(checks);
  const blocking = checks.filter((c) => !c.ok && c.fatal);
  if (blocking.length && !dryRun) {
    throw new Error(`Install cannot continue: ${blocking.map((c) => c.label).join(', ')}`);
  }
}

/** Inside a container, localhost is the container itself. */
function warnLocalDatabase(answers: Answers, databaseUrl = ''): void {
  if (answers.host !== 'docker' || !/(localhost|127\.0\.0\.1)/.test(databaseUrl)) return;
  warn('DATABASE_URL points at localhost, which inside a container means the container itself.');
  note('Use host.docker.internal (macOS, Windows) or the host IP so the server can reach it.');
}

/** Only a fresh KEK can orphan existing data; reusing .env's is always safe. */
async function guardKek(existing: Record<string, string>, answers: Answers, root: string, dryRun: boolean) {
  if (existing.OYA_PROFILE_SECRET || answers.host !== 'docker' || dryRun) return;
  const volume = await priorState();
  if (!volume) return;
  const reused = await resolveKekConflict(volume, root);
  if (reused) existing.OYA_PROFILE_SECRET = reused;
}

/** Everything decided, checked and resolved, before anything is written. */
async function plan(flags: Record<string, string | boolean>, dryRun: boolean): Promise<Plan> {
  const interviewed = await gather(typeof flags.config === 'string' ? flags.config : null);
  const { root, answers, secrets } = interviewed;
  await checkMachine(root, answers, dryRun);
  warnLocalDatabase(answers, secrets.DATABASE_URL);
  const envPath = join(root, '.env');
  const existing = readEnv(envPath);
  await guardKek(existing, answers, root, dryRun);
  return { ...interviewed, envPath, existing, env: buildEnv(answers, secrets, existing) };
}

/** Writes .env and the answers file, warning when the KEK is new. */
function writeFiles(p: Plan): void {
  writeEnv(p.envPath, renderEnv(p.env.values, p.env.extra));
  writeFileSync(join(p.root, ANSWERS_FILE), JSON.stringify(p.answers, null, JSON_INDENT) + '\n');
  success(`wrote ${p.envPath}`);
  if (p.existing.OYA_PROFILE_SECRET) return;
  warn('OYA_PROFILE_SECRET was generated — back it up.');
  note('Without it, stored cookies, proxy credentials and TOTP seeds cannot be decrypted.');
}

/** Provisions the k8s fleet; the runtime rejects a tag, so .env must carry the resolved digest. */
async function provisionK8s(p: Plan): Promise<void> {
  const pinned = await provisionK8sFleet(p.root, p.answers);
  if (pinned === p.env.values.OYA_MANAGED_IMAGE) return;
  p.env.values.OYA_MANAGED_IMAGE = pinned;
  writeEnv(p.envPath, renderEnv(p.env.values, p.env.extra));
  console.log('  .env updated with the pinned image digest');
}

/** Writes, migrates, provisions and waits for the server. */
async function apply(p: Plan): Promise<void> {
  writeFiles(p);
  if (p.migrateUrl) await migrate(p.root, p.migrateUrl);
  if (p.answers.fleet === 'k8s') await provisionK8s(p);
  if (p.answers.host === 'docker') await provisionDocker(p.root, p.answers);
  await waitReady(p.answers.publicUrl);
  printDone(p.answers.publicUrl, p.env.apiKey);
}

/** `oya install [--dry-run] [--config oya-install.json]`. */
export async function cmdInstall(flags: Record<string, string | boolean>): Promise<void> {
  const dryRun = flags['dry-run'] === true;
  const p = await plan(flags, dryRun);
  printPlan(p.answers, p.envPath);
  if (dryRun) return printDryRun(p.env.values);
  await apply(p);
}
