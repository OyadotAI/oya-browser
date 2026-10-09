/** Browser-owned input capabilities; no protocol adapter belongs in this module. */
export { nativePointer, nativeWheel, type NativePointerView } from './native-pointer.ts';
export { nativeDrag, type NativeDragView, type DragPoint, type DragKind } from './native-drag.ts';
export { isNativeKeyDispatch } from './native-key-dispatch.ts';
