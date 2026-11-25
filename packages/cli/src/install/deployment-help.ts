/** Resolve explicit field guidance without discovering or printing an operator's credentials. */
import { presentQuestion, type QuestionHelp, type QuestionPresentation } from '../prompt.ts';
import type { DeploymentOptions, DeploymentTarget } from './deployment-targets.ts';
import { BASIC_HELP } from './deployment-help-basic.ts';
import { ADVANCED_HELP } from './deployment-help-advanced.ts';

/** Title, purpose, acquisition path, example, default behavior and optional read-only lookup command. */
export type HelpEntry = readonly [string, string, string, string, string, string?];

/** Explicit catalog coverage makes a missing help entry a development error rather than vague advice. */
const HELP = { ...BASIC_HELP, ...ADVANCED_HELP };

/** Shell quoting keeps displayed lookup commands safe to copy even with unusual context names. */
const quoted = (value: string): string => `'${value.replaceAll("'", "'\\''")}'`;

/** Substitute only previously selected, nonsecret destination values. */
function commandFor(command: string | undefined, options: DeploymentOptions): string | undefined {
  return command?.replace(/\{(context|namespace|region|project)\}/g, (_, key: string) =>
    quoted(String(options[key] || key.toUpperCase())),
  );
}

/** Return all the guidance needed for one normal, Advanced or synthetic question. */
export function deploymentHelp(target: DeploymentTarget, field: string, options: DeploymentOptions = {}): QuestionHelp {
  const key = Object.hasOwn(HELP, `${target}:${field}`) ? `${target}:${field}` : field;
  if (!Object.hasOwn(HELP, key)) throw new Error(`Missing setup guidance for ${field}.`);
  const [title, purpose, find, example, behavior, command] = HELP[key];
  const question = field === 'target' ? 'Where should Oya run?' : undefined;
  return { title, question, purpose, find, example, behavior, command: commandFor(command, options) };
}

/** Each question receives a separate renderer and cannot erase a different question's transcript. */
export function deploymentQuestion(
  target: DeploymentTarget,
  field: string,
  options: DeploymentOptions = {},
): QuestionPresentation {
  return presentQuestion(deploymentHelp(target, field, options));
}
