/** Workflow vocabulary dispatch through browser-owned native page, DOM, frame and input operations. */
import type { Step } from '../../workflow/index.ts';
import type { PageDriver, DriverTab } from '../actions/driver.ts';
import { actionNavigation } from '../actions/action-navigation.ts';
import { nativeCommand, nativeParams } from './native-actions.ts';
import { clickElement, hoverElement, typeElement, selectElement, type ElementInput } from './native-element-input.ts';
import { targetOperation, type NativeSelection } from './native-target-operations.ts';
import { pressWorkflowKey } from './native-keys.ts';
import { nativeValue } from './native-preflight.ts';
import type { WorkflowFile } from './native-files.ts';
/** One admitted step retains run control until all native work settles. */
export interface Execution {
  /** Actual production page devices. */ driver: PageDriver;
  /** Exact run-owned native tab. */ tab: DriverTab;
  /** Immutable step. */ step: Step;
  /** Immutable substitutions. */ vars: Record<string, unknown>;
  /** Exact selected node, for targeted actions. */ selected?: NativeSelection;
  /** Preflight-approved immutable file bytes. */ files: Map<string, WorkflowFile[]>;
  /** Stop and exact-tab fence. */ guard(): void;
}
/** Interchangeable action strategies never accept an arbitrary browser command. */
const ACTIONS: Record<string, (run: Execution) => Promise<void>> = {
  navigate: (run) => command(run, 'navigate'),
  go_back: (run) => command(run, 'back'),
  go_forward: (run) => command(run, 'forward'),
  scroll: (run) => command(run, 'scroll'),
  click: (run) => clickElement(element(run)),
  double_click: (run) => clickElement(element(run), true),
  hover: (run) => hoverElement(element(run)),
  type: (run) => typeElement(element(run), nativeValue(run.step.text, run.vars)),
  select_option: (run) => selectElement(element(run), nativeValue(run.step.option, run.vars)),
  press_key: (run) =>
    actionNavigation(run.tab.view, () =>
      pressWorkflowKey(run.driver, run.tab, nativeValue(run.step.key, run.vars)),
    ).then(() => {}),
  upload_file: (run) => upload(run),
};
/** Typed exact-target input context, with no active-tab lookup. */
function element(run: Execution): ElementInput {
  if (!run.selected) throw Error('Workflow action requires an exact target');
  return { driver: run.driver, target: run.selected, guard: run.guard };
}
/** Reuse existing navigation protection and native history behavior on the pinned tab. */
async function command(run: Execution, action: string): Promise<void> {
  await nativeCommand(run.driver, run.tab, { action, params: nativeParams(run.step, run.vars) });
}
/** Native isolated FileList assignment is explicit DOM upload semantics, never a claimed native chooser gesture. */
async function upload(run: Execution): Promise<void> {
  const input = element(run),
    files = run.files.get(run.step.id);
  if (!files) throw Error('Workflow files were not authorized during preflight');
  input.guard();
  await targetOperation(input.target, 'file');
  input.guard();
  await targetOperation(input.target, 'upload', { files });
  input.guard();
}
/** Every emitted operation belongs to the admitted step and exact browser-owned native target. */
export async function executeNative(run: Execution): Promise<void> {
  if (!Object.hasOwn(ACTIONS, run.step.action)) throw Error('Unsupported native workflow action');
  run.guard();
  await ACTIONS[run.step.action](run);
}
