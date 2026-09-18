/**
 * What the wizard prints: the preflight results, the plan, a dry run's
 * settings (secrets hidden), and how to sign in once the stack is up.
 */
import { banner, style, icon } from '../prompt.ts';
import { SECRET_KEYS } from './env-file.ts';
import type { Answers, Check } from './types.ts';
import { PLAN_LABEL_WIDTH } from './constants.ts';

/** Every check, marked ok, failed (fatal) or warned. */
export function printPreflight(checks: Check[]): void {
  console.log(`\n${style.bold('  Preflight')}`);
  for (const c of checks) {
    const mark = c.ok ? icon.ok : c.fatal ? icon.fail : icon.warn;
    console.log(`  ${mark} ${c.label}${c.detail ? style.grey(`  — ${c.detail}`) : ''}`);
  }
}

/** One labelled line of the plan. */
const row = (k: string, v: string) => console.log(`  ${style.grey(k.padEnd(PLAN_LABEL_WIDTH))}${v}`);

/** What will run, and what will be written where. */
export function printPlan(answers: Answers, envPath: string): void {
  console.log(`\n${style.bold('  Plan')}`);
  const { llm } = answers;
  row('control', `${answers.host} · ${answers.database}`);
  row('browsers', `${answers.fleet}${answers.workers ? ` ×${answers.workers}` : ''}`);
  row('llm', llm.provider === 'skip' ? style.grey('none') : `${llm.model} ${style.grey(`@ ${llm.baseUrl}`)}`);
  row('writes', `${envPath} ${style.grey('(0600)')}`);
  row('then', `docker compose up -d, wait for ${answers.publicUrl}/readyz`);
}

/** A dry run's settings, with every secret shown as hidden. */
export function printDryRun(values: Record<string, string>): void {
  console.log('\n--dry-run: nothing was written. Settings that would be applied:');
  for (const k of Object.keys(values)) {
    console.log(`  ${k}=${SECRET_KEYS.has(k) ? style.grey('<hidden>') : values[k]}`);
  }
}

/** The stack is up: the key, and the commands to start using it. */
export function printDone(publicUrl: string, apiKey: string): void {
  banner(`${publicUrl} is up`, 'Sign in to the dashboard with the key below.');
  console.log(`\n  ${style.bold('API key')}  ${style.green(apiKey)}\n`);
  console.log(`  ${style.grey('# your shell may export OYA_API_KEY for the hosted service')}`);
  console.log(`  unset OYA_API_KEY`);
  console.log(`  oya login --url ${publicUrl} --key ${apiKey}`);
  console.log(`  oya ls && oya goto https://example.com\n`);
}
