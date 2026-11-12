/** Adapt the setup wizard to the maintained examples, while leaving the original quickstart compatible. */
import { renameSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { readJsonFile, type Flags } from '../args.ts';
import { note, steps, setupIntro, setupPhase, setupAnswer } from '../prompt.ts';
import { locateRepo } from './repo.ts';
import { deploymentTarget, optionArguments, destinationOptions, DEPLOYMENT_TARGETS } from './deployment-targets.ts';
import { parseDeploymentPlan, type DeploymentPlan } from './deployment-plan.ts';
import {
  chooseDeploymentTarget,
  deploymentDatabase,
  interviewDeployment,
  showContextHint,
} from './deployment-interview.ts';
import { deploymentPreflight } from './deployment-preflight.ts';
import { runDeploymentScript } from './deployment-process.ts';
import {
  printDeploymentPlan,
  printDeploymentChecks,
  printDeploymentDone,
  printDeploymentIncomplete,
} from './deployment-report.ts';
import { DEPLOYMENT_PLAN_VERSION, ENV_FILE_MODE, JSON_INDENT } from './constants.ts';
import type { Check } from './types.ts';

/** This file is ignored by git and never contains credentials. */
const PLAN_FILE = 'oya-install.json';

/** Read only the format discriminator until its full validator runs. */
interface VersionedPlan {
  /** Legacy answer format or current deployment format. */
  version?: unknown;
}

/** Select legacy replay deliberately instead of silently coercing malformed versioned plans. */
function savedPlan(flags: Record<string, string | boolean>): unknown {
  if (typeof flags.config !== 'string') return null;
  const plan = readJsonFile(flags.config) as VersionedPlan | null;
  if (!plan || (plan.version !== 1 && plan.version !== DEPLOYMENT_PLAN_VERSION))
    throw new Error('Unsupported install configuration version. Expected version 1 or 2.');
  if (plan.version === 1 && flags.target) throw new Error('--target cannot override a version 1 saved installation.');
  return plan;
}

/** Preserve bare --yes and the original saved-answer format without asking a second target question. */
function legacy(flags: Record<string, string | boolean>, saved: unknown): boolean {
  if (saved) return (saved as VersionedPlan).version === 1;
  return flags.yes === true && !flags.target;
}

/** A replay is complete and prompt-free; a shortcut selects only a target, leaving its questions visible. */
async function gatherDeployment(flags: Record<string, string | boolean>, saved: unknown): Promise<DeploymentPlan> {
  if (saved) return replayDeployment(flags, saved);
  const target = flags.target ? deploymentTarget(flags.target) : await chooseDeploymentTarget();
  if (flags.target) setupAnswer('Target', DEPLOYMENT_TARGETS[target].label);
  if (!flags.yes) await showContextHint(target);
  return interviewDeployment(target, flags.yes === true);
}

/** A target shortcut cannot redirect a saved installation to another cloud. */
function replayDeployment(flags: Record<string, string | boolean>, saved: unknown): DeploymentPlan {
  const plan = parseDeploymentPlan(saved);
  if (flags.target && flags.target !== plan.target)
    throw new Error('--target conflicts with the saved deployment target.');
  setupAnswer('Target', DEPLOYMENT_TARGETS[plan.target].label);
  setupPhase('Configure');
  return plan;
}

/** Atomically retain nonsecret answers for recovery, even when prerequisite checks fail. */
function saveDeploymentPlan(root: string, p: DeploymentPlan): void {
  const path = join(root, PLAN_FILE),
    temporary = `${path}.${process.pid}.tmp`;
  writeFileSync(temporary, `${JSON.stringify(p, null, JSON_INDENT)}\n`, { mode: ENV_FILE_MODE, flag: 'wx' });
  renameSync(temporary, path);
  note(`Saved ${path}; credentials stay in the target's secret store.`);
}

/** The scripts own durable secrets and migration; the adapter forwards only an ephemeral database input. */
async function executeDeployment(root: string, p: DeploymentPlan, databaseUrl: string): Promise<void> {
  try {
    await deployAndVerify(root, p, databaseUrl);
  } catch (err) {
    printDeploymentIncomplete(p, root);
    throw err;
  }
  printDeploymentDone(p, root);
}

/** A single script owns deployment and verification against the same destination. */
async function deployAndVerify(root: string, p: DeploymentPlan, databaseUrl: string): Promise<void> {
  const script = join(root, 'deployments', p.target, 'deploy.sh');
  const env = deploymentEnvironment(p, databaseUrl);
  setupPhase('Deploy');
  await runDeploymentScript(script, ['deploy', ...optionArguments(p.options)], root, env);
  setupPhase('Verify');
  note('Verifying control-plane readiness and a complete browser session…');
  await runDeploymentScript(script, ['smoke', ...optionArguments(destinationOptions(p.target, p.options))], root, env);
}

/** Credentials are an ephemeral child environment, never part of command arguments or saved answers. */
function deploymentEnvironment(p: DeploymentPlan, databaseUrl: string): NodeJS.ProcessEnv {
  return {
    ...process.env,
    OYA_INSTALLER: '1',
    OYA_DEPLOY_DATABASE_URL: databaseUrl,
    OYA_DEPLOY_PUBLIC_URL: p.publicUrl || '',
  };
}

/** Start the target-first interface without borrowing the legacy six-step counter. */
function announceDeployment(): void {
  setupIntro();
  steps(0);
}

/** Return false only for the original install path; all new paths either finish or fail explicitly. */
export async function installDeployment(flags: Record<string, string | boolean>): Promise<boolean> {
  const saved = savedPlan(flags);
  if (legacy(flags, saved)) return false;
  if (flags.target) deploymentTarget(flags.target);
  announceDeployment();
  const plan = await gatherDeployment(flags, saved);
  const root = await locateRepo(flags['dry-run'] === true, true);
  await applyDeployment(root, plan, flags, !!saved);
  return true;
}

/** Preview before writes, then retain the plan, inspect fatal failures and invoke its existing example. */
async function applyDeployment(root: string, p: DeploymentPlan, flags: Flags, replay: boolean): Promise<void> {
  const checks = await deploymentPreflight(root, p, flags['dry-run'] === true);
  showDeployment(p, root, checks);
  if (flags['dry-run'] === true) return printPreview();
  saveDeploymentPlan(root, p);
  if (checks.some((c) => !c.ok && c.fatal))
    throw new Error('Cannot deploy: fix the failed preflight checks, then rerun the saved plan.');
  const databaseUrl = await deploymentDatabase(p, replay || flags.yes === true);
  await executeDeployment(root, p, databaseUrl);
}

/** Keep the reviewed destination and its prerequisite evidence together. */
function showDeployment(p: DeploymentPlan, root: string, checks: Check[]): void {
  printDeploymentPlan(p, root);
  printDeploymentChecks(checks);
}

/** Make the preview's side-effect and verification limits unambiguous. */
function printPreview(): void {
  note('Preview only. No files or resources changed. Database credentials are hidden and checked during deployment.');
}
