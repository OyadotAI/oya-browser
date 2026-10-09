/** Fixed native input commands shared by external adapters; no arbitrary event forwarding. */
import type { NativePage } from './page.ts';
import { insertNativeText } from './text-input.ts';
import { dispatchNativePointer } from './pointer-input.ts';
import { dispatchNativeKey } from './keyboard-input.ts';
/** Only explicitly implemented native operations can be selected. */
const COMMANDS = {
  'input:pointer': dispatchNativePointer,
  'input:key': dispatchNativeKey,
  'input:text': (page: NativePage, params: Record<string, unknown>) => insertNativeText(page, params.text as string),
};
/** The command's already-authorized exact page remains its destination for the whole dispatch. */
export function dispatchNativeInput(page: NativePage, action: string, params: Record<string, unknown>) {
  if (!Object.hasOwn(COMMANDS, action)) throw Error('Unsupported native input operation');
  return COMMANDS[action as keyof typeof COMMANDS](page, params);
}
