/** Repeated execution follows a known task through reusable code, exceptions and an appropriate deployment. */
import type { AnswerSection } from './content';
/** Workflow pages supply their own first case before these common consequences. */
export const WORKFLOW_CHAPTERS: AnswerSection[] = [
  {
    heading: 'Save the steps as code you can inspect.',
    body: 'A successful run can become a playbook: readable Playwright code with the changing details saved as variables.',
    layout: 'flow',
  },
  {
    heading: 'Run the next case with new inputs.',
    body: 'Replay those saved steps without a model in the loop. The inputs change; the recorded workflow supplies the steps.',
    layout: 'flow',
  },
  {
    heading: 'Plan for the cases that need attention.',
    body: 'Portal changes can stop a replay, and some sessions require sign-in again. An agent can try a repair; server repairs can be saved, while suggested desktop changes need review before use.',
  },
  {
    heading: 'Choose a deployment for the data involved.',
    body: 'For protected health information (PHI), use the covered Oya Cloud or enterprise offering and request the Business Associate Agreement (BAA). Connected providers have different runtime guarantees; HIPAA mode uses Oya’s own browsers.',
    layout: 'choices',
  },
];
/** Infrastructure readers start with a working task and then consider execution and ownership. */
export const INFRASTRUCTURE_CHAPTERS: AnswerSection[] = [
  {
    heading: 'Begin with one task that works.',
    body: 'An agent completes the task once. Save its successful steps as a playbook: readable code with the changing inputs made into variables.',
    layout: 'flow',
  },
  {
    heading: 'Give each run its inputs and a browser.',
    body: 'Replay the saved steps with new inputs. Use queues and concurrency controls to manage the work admitted to your browser fleet.',
    layout: 'flow',
  },
  {
    heading: 'Keep access and intervention explicit.',
    body: 'Set project roles and credentials, inspect runs and let a person take control when needed.',
    layout: 'controls',
  },
  {
    heading: 'Choose the deployment that fits the data.',
    body: 'Use Oya Cloud or your infrastructure. Runtime guarantees vary by provider. For protected health information (PHI), use the covered Cloud or enterprise offering under a Business Associate Agreement (BAA); HIPAA mode keeps PHI on Oya’s own browsers.',
    layout: 'choices',
  },
];
