/**
 * The landing page diagram's three stages: record, replay, repair. Content
 * only; workflow-diagram.tsx draws it.
 */
import { MousePointer2, Repeat2, Wrench } from 'lucide-react';

/** The repair stage, drawn with a draft output and a wand on its middle step. */
export const REPAIR_STAGE = 2;
/** The step the agent repairs. */
export const REPAIRED_STEP = 1;

/** Each stage: its tab, its canvas and its code. */
export const stages = [
  {
    name: 'Record',
    icon: MousePointer2,
    kicker: 'A task becomes a workflow',
    input: '“Prepare a portal request.”',
    steps: ['Navigate', 'Fill', 'Review'],
    output: 'Save the successful run',
    code: 'browser.toPlaybook("portal-request-review")',
    note: 'The agent does the task once. You keep the steps.',
  },
  {
    name: 'Replay',
    icon: Repeat2,
    kicker: 'Same workflow. New inputs.',
    input: '{ requestId: "DEMO-0002" }',
    steps: ['Navigate', 'Fill', 'Review'],
    output: 'Run the recorded steps',
    code: 'browser.play("portal-request-review", data)',
    note: 'Recorded steps replay without an LLM in the loop.',
  },
  {
    name: 'Repair',
    icon: Wrench,
    kicker: 'A changed page is a review point',
    input: 'The portal form changed.',
    steps: ['Step breaks', 'Agent repairs', 'You review'],
    output: 'Keep the fix as a draft',
    code: 'oya.playbooks.promote("portal-request-review")',
    note: 'Auto-healing saves a draft. Promote it after review.',
  },
];
