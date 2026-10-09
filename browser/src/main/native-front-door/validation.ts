/** The supported subset rejects semantic options it cannot honor, rather than silently weakening isolation. */
import { COOKIE_PARAMS, validateCookies } from './validation-cookies.ts';
import { NETWORK_PARAMS, validateNetwork } from './validation-network.ts';
import { BROWSER_PARAMS, validateBrowser } from './validation-browser.ts';
import { metricParameters } from '../native/index.ts';
import { RUNTIME_PARAMS, validateRuntime } from './validation-runtime.ts';
import { validateTargets } from './validation-targets.ts';
import { validateDom } from './validation-dom.ts';
import type { NativeCommand } from './types.ts';
/** Allowed argument names are explicit for every supported method. */
const PARAMS: Record<string, readonly string[]> = {
  ...COOKIE_PARAMS,
  ...RUNTIME_PARAMS,
  ...BROWSER_PARAMS,
  ...NETWORK_PARAMS,
  'Emulation.setDeviceMetricsOverride': ['width', 'height', 'deviceScaleFactor', 'mobile'],
  'Emulation.clearDeviceMetricsOverride': [],
  'Page.getFrameTree': [],
  'Page.enable': [],
  'Page.disable': [],
  'Page.reload': ['ignoreCache'],
  'Page.stopLoading': [],
  'DOM.getDocument': ['depth', 'pierce'],
  'DOM.querySelector': ['nodeId', 'selector'],
  'DOM.querySelectorAll': ['nodeId', 'selector'],
  'DOM.describeNode': ['nodeId', 'depth', 'pierce'],
  'DOM.getAttributes': ['nodeId'],
  'DOM.getOuterHTML': ['nodeId'],
  'DOM.focus': ['nodeId'],
  'DOM.scrollIntoViewIfNeeded': ['nodeId'],
  'Input.insertText': ['text'],
  'Target.setAutoAttach': ['autoAttach', 'flatten', 'waitForDebuggerOnStart', 'filter'],
  'Target.setDiscoverTargets': ['discover'],
  'Target.activateTarget': ['targetId'],
  'Page.bringToFront': [],
  'Log.enable': [],
  'Log.disable': [],
  'Browser.getVersion': [],
  'Target.getTargets': [],
  'Target.getTargetInfo': ['targetId'],
  'Target.createTarget': ['url', 'browserContextId'],
  'Target.createBrowserContext': ['disposeOnDetach'],
  'Target.getBrowserContexts': [],
  'Target.disposeBrowserContext': ['browserContextId'],
  'Target.closeTarget': ['targetId'],
  'Target.attachToTarget': ['targetId', 'flatten'],
  'Target.detachFromTarget': ['sessionId'],
  'Page.navigate': ['url'],
  'Page.captureScreenshot': ['format'],
  'Oya.getCapabilities': [],
  'Oya.getNavigationHistory': [],
  'Oya.navigateToHistoryEntry': ['snapshot', 'index'],
  'Oya.analyze': ['format'],
  'Oya.click': ['selector'],
  'Oya.type': ['selector', 'text'],
  'Oya.pressKey': ['key'],
  'Oya.scroll': ['direction', 'amount'],
  'Oya.readElements': ['selector', 'limit'],
};
/** Reject unknown or mistyped arguments before touching native state. */
export function validateParams(command: NativeCommand): void {
  if (!Object.hasOwn(PARAMS, command.method)) return;
  for (const [key, value] of Object.entries(command.params)) {
    if (!PARAMS[command.method].includes(key)) throw Error(`Unsupported parameter: ${key}`);
    validatePrimitive(command.method, key, value);
  }
  validateRequired(command);
  if (command.method.startsWith('DOM.')) validateDom(command);
  if (command.method === 'Emulation.setDeviceMetricsOverride') metricParameters(command.params);
}
/** The small subset has string arguments, finite numeric bounds, and one boolean attachment flag. */
function validateValue(key: string, value: unknown): void {
  if (['flatten', 'discover', 'ignoreCache', 'disposeOnDetach'].includes(key) && typeof value === 'boolean') return;
  if (['amount', 'limit'].includes(key) && typeof value === 'number' && Number.isFinite(value) && value > 0) return;
  if (
    !['flatten', 'discover', 'ignoreCache', 'disposeOnDetach', 'amount', 'limit', 'index'].includes(key) &&
    typeof value === 'string'
  )
    return;
  throw Error(`Invalid parameter: ${key}`);
}

/** Structured method arguments have their own validators rather than primitive coercion. */
function validatePrimitive(method: string, key: string, value: unknown): void {
  if (
    !Object.hasOwn(COOKIE_PARAMS, method) &&
    !Object.hasOwn(BROWSER_PARAMS, method) &&
    !Object.hasOwn(NETWORK_PARAMS, method) &&
    !['Target.setAutoAttach', 'Oya.navigateToHistoryEntry'].includes(method) &&
    !/^(DOM|Emulation|Runtime)\./.test(method)
  )
    validateValue(key, value);
}
/** Required structured arguments cannot disappear through an empty parameter object. */
function validateRequired(command: NativeCommand): void {
  validateHistoryCommand(command);
  validateCookies(command);
  validateTargets(command);
  validateBrowser(command);
  validateNetwork(command);
  validateRuntime(command);
  if (command.method === 'Input.insertText' && typeof command.params.text !== 'string') throw Error('text is required');
}

/** History traversal requires an explicit snapshot and nonnegative native entry position. */
function validateHistoryCommand({ method, params }: NativeCommand): void {
  if (method !== 'Oya.navigateToHistoryEntry') return;
  if (typeof params.snapshot !== 'string' || !params.snapshot) throw Error('snapshot is required');
  if (!Number.isSafeInteger(params.index) || (params.index as number) < 0) throw Error('Invalid native history index');
}

/** Introspection reuses the actual validator table and returns a copy, never mutable policy state. */
export function nativeParameterNames(method: string): string[] {
  if (!Object.hasOwn(PARAMS, method)) throw Error('Missing native method parameter contract');
  return [...PARAMS[method]];
}
