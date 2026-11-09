/** Synthetic examples derived from existing workflows; these records never represent live patient data. */
import { REPLAY, WORKFLOWS } from './content';
import type { ReplayField, ReplayTask, Workflow } from './content';

/** Only these existing tasks appear in the compact product scene. */
const DEMO_COUNT = 3;
/** Width of an illustrative audit reference's numeric suffix. */
const REFERENCE_WIDTH = 4;

/** Each prompt is paired with its task's label rather than inferred from selection order. */
interface ExampleCopy {
  /** The tab's concise workflow name. */ label: string;
  /** The illustrative natural-language request for this exact workflow. */ prompt: string;
}

/** A selected descriptor keeps its prompt, artifact, inputs and result consistent. */
export interface DemoExample extends ExampleCopy {
  /** Existing task data that drives every visible part of the replay. */ task: ReplayTask;
}

/** Synthetic prompts refer only to identifiers and fields already present in each existing example. */
const EXAMPLE_COPY: Record<string, ExampleCopy> = {
  'prior-auth': {
    label: 'Prior authorization',
    prompt: 'Submit a prior authorization for member W2847-1193, CPT 72148.',
  },
  eligibility: { label: 'Eligibility', prompt: 'Check eligibility and benefits for member W2847-1193.' },
  claims: { label: 'Claim status', prompt: 'Find the status and remittance details for claim CLM-4471902.' },
};

/** Uses a field's existing label and selection type in the illustrative exported code. */
function fieldCode({ label, select }: ReplayField) {
  return `await page.getByLabel(${JSON.stringify(label)}).${select ? 'selectOption' : 'fill'}(vars[${JSON.stringify(label)}]);`;
}

/** A generated example uses the selected form's labels, submit action and result. */
function codeFor(workflow: Workflow) {
  return [
    'await page.goto(vars["portal"]);',
    ...workflow.form.fields.map(fieldCode),
    `await page.getByRole("button", { name: ${JSON.stringify(workflow.form.button)} }).click();`,
    `await expect(page.getByText(${JSON.stringify(workflow.form.result)})).toBeVisible();`,
  ];
}

/** Describes this task's own actions rather than reusing prior-authorization evidence. */
function actionsFor(workflow: Workflow) {
  return [
    'Opened payer portal',
    ...workflow.form.fields.map(({ label }) => `Filled ${label}`),
    workflow.form.button,
    workflow.form.result,
  ];
}

/** Explicitly fictional references keep example tasks distinct; they are not cryptographic evidence. */
function ledgerFor(workflow: Workflow) {
  return actionsFor(workflow).map((action, index) => ({
    time: REPLAY.ledger[index].time,
    actor: `agent · ${workflow.id}`,
    action,
    hash: `${workflow.id === 'eligibility' ? 'e4' : 'c8'}${index.toString().padStart(REFERENCE_WIDTH, '0')}`,
  }));
}

/** Keeps the original prior-auth example and builds task-correct alternatives from existing data. */
function taskFor(workflow: Workflow): ReplayTask {
  if (workflow.id === 'prior-auth') return REPLAY;
  return {
    ...workflow.form,
    address: `portal.payer.example/${workflow.id}`,
    file: `${workflow.id}.ts`,
    code: codeFor(workflow),
    ledger: ledgerFor(workflow),
  };
}

/** Missing editorial data must be corrected rather than silently showing another task's prompt. */
function exampleFor(workflow: Workflow): DemoExample {
  if (!Object.hasOwn(EXAMPLE_COPY, workflow.id)) throw new Error('Missing demo workflow copy');
  return { task: taskFor(workflow), ...EXAMPLE_COPY[workflow.id] };
}

/** Stable descriptors let a task remount and release the previous replay's timer. */
export const DEMO_EXAMPLES = WORKFLOWS.slice(0, DEMO_COUNT).map(exampleFor);
/** Existing task exports remain available to callers and behavior tests. */
export const DEMO_TASKS = DEMO_EXAMPLES.map(({ task }) => task);
/** Existing label exports remain available while the descriptor owns their relationship. */
export const DEMO_LABELS = DEMO_EXAMPLES.map(({ label }) => label);
