/** Literal browser-owned picker elements and PNG previews, shared by cards and final confirmation. */
import type { ShareOption } from '../shared/share-picker.ts';

/** Resolve only fixed elements from the packaged private picker document. */
export const pickerElement = (id: string): HTMLElement => document.getElementById(id)!;
/** Source metadata is always literal text, never page-controlled markup. */
export function pickerText(tag: string, className: string, value: string): HTMLElement {
  const node = document.createElement(tag);
  node.className = className;
  node.textContent = value;
  return node;
}
/** A missing thumbnail remains an accessible placeholder without invalidating a verified window. */
export function pickerPreview(option: ShareOption): HTMLElement {
  const box = pickerText('span', 'preview placeholder', 'Preview unavailable');
  if (!option.thumbnail.startsWith('data:image/png;base64,') || option.thumbnail.endsWith(',')) return box;
  loadPickerPreview(box, option.thumbnail);
  return box;
}
/** Keep the placeholder until browser-produced PNG bytes have decoded. */
function loadPickerPreview(box: HTMLElement, thumbnail: string): void {
  const image = document.createElement('img');
  image.alt = '';
  image.addEventListener('load', () => {
    box.className = 'preview';
    box.replaceChildren(image);
  });
  image.src = thumbnail;
}
