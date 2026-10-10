/** Execute workflow actions against one exact run-owned native tab. */
import { PageDriver, type DriverTab } from '../actions/driver.ts';
import type { CommandParams } from '../actions/types.ts';
import type { Step } from '../../workflow/index.ts';
import { nativeValue } from './native-preflight.ts';
import type { TargetRead } from './native-target.ts';
/** Exact native input surface. */
interface Surface {
  /** Production native driver. */ driver: PageDriver;
  /** Pinned tab. */ tab: DriverTab;
}
/** Field selection and cancellation boundary. */
export interface NativeField {
  /** Exact CSS. */ selector: string;
  /** Text replacement. */ text: string;
  /** Inspection. */ target: TargetRead;
  /** Recheck run identity and cancellation. */ guard(): void;
}
/** Clone the facade without changing application-global active-tab or response routing. */
function pinnedDriver(source: PageDriver, tab: DriverTab, respond: PageDriver['deps']['sendResult']): PageDriver {
  return new PageDriver({
    ...source.deps,
    ...{ keyboard: source.keyboard, mouse: source.mouse },
    getActiveView: () => tab.view,
    tabs: () => [tab],
    activeTabId: () => tab.id,
    sendResult: respond,
  });
}
/** A validated native workflow action. */
interface NativeCommand {
  /** Production action name. */ action: string;
  /** Resolved parameters. */ params: CommandParams;
}
/** Existing production commands check protection, egress and covered targets. */
export async function nativeCommand(source: PageDriver, tab: DriverTab, command: NativeCommand): Promise<void> {
  const response = new NativeResponse();
  const driver = pinnedDriver(source, tab, response.receive);
  await driver.runPageAction('native-workflow', command.action, command.params, tab.view);
  response.check();
}
/** Clear the intended editable field even for empty replacements, using native editing and input. */
export async function nativeType({ driver, tab }: Surface, field: NativeField): Promise<void> {
  const { selector, text, target, guard } = field;
  await focusNativeField({ driver, tab }, { selector, target, guard });
  guard();
  await driver.keyboard.clear(tab.view);
  guard();
  await driver.keyboard.type(tab.view, text);
}
/** Translate workflow vocabulary without forwarding arbitrary action names. */
export function nativeParams(step: Step, vars: Record<string, unknown>, selector?: string): CommandParams {
  return {
    selector,
    url: nativeValue(step.url, vars),
    key: nativeValue(step.key, vars),
    direction: step.direction,
    amount: step.amount,
  };
}
/** Assertions compare normalized text and exact values without mutating the page. */
export function nativeAssertion(step: Step, expected: string, target: TargetRead | undefined, url: string): boolean {
  if (step.action === 'assert_url') return url === expected;
  if (step.action === 'assert_page') return samePage(url, expected, step.params);
  if (step.action === 'assert_text') return target?.text === expected.replace(/\s+/g, ' ').trim();
  if (step.action === 'assert_value') return target?.value === expected;
  return target?.count === 1;
}
/** Recorded page assertions ignore session queries except explicitly selected parameters. */
function samePage(actual: string, expected: string, params = ''): boolean {
  const a = new URL(actual),
    e = new URL(expected);
  const pathname = (u: URL): string => u.pathname.replace(/\/ref=[^/]*/g, '');
  const pathMatches = a.origin === e.origin && pathname(a) === pathname(e);
  const hashMatches = !e.hash.startsWith('#/') || a.hash === e.hash;
  return pathMatches && hashMatches && sameParams(a, e, params);
}
/** Compare only query fields explicitly pinned by the recorded assertion. */
function sameParams(actual: URL, expected: URL, params: string): boolean {
  return params
    .split(',')
    .filter(Boolean)
    .every((key) => actual.searchParams.get(key) === expected.searchParams.get(key));
}
/** One action must explicitly acknowledge completion, including failure replies. */
class NativeResponse {
  /** Missing acknowledgement is a failure. */ private failure: string | undefined =
    'Native action did not acknowledge completion';
  /** Native facade response without touching server command responses. */
  readonly receive: PageDriver['deps']['sendResult'] = (_id, ok, _data, error) => {
    this.failure = ok ? undefined : error || 'Native action failed';
  };
  /** Refuse a missing or failed response. */
  check(): void {
    if (this.failure) throw new Error(this.failure);
  }
}

/** Native focus refuses covered targets and rechecks cancellation after locating. */
async function focusNativeField(
  { driver, tab }: Surface,
  field: Pick<NativeField, 'selector' | 'target' | 'guard'>,
): Promise<void> {
  if (!field.target.editable) throw new Error('Unsupported native workflow editable target');
  const spot = await driver.locate(tab.view, field.selector);
  if (!spot?.ok || spot.data.covered) throw new Error('Workflow target is missing or covered');
  field.guard();
  await driver.mouse.click(tab.view, spot.data.x, spot.data.y);
}
