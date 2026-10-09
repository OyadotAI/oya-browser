/** Browser-owned input capabilities; no protocol adapter belongs in this module. */
export { nativePointer, nativePointerExact, nativeWheel, type NativePointerView } from './native-pointer.ts';
export { nativeDrag, type NativeDragView, type DragPoint, type DragKind } from './native-drag.ts';
export { isNativeKeyDispatch } from './native-key-dispatch.ts';
export { sendNativeKey } from './native-key-dispatch.ts';
export { keyDef } from './keyboard.ts';
export { nativeKeyName } from './native-keyboard.ts';
