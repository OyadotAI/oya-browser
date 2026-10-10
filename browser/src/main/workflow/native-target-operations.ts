/** Exact isolated node operations retain identity across all native input awaits. */
import type { NativeScope, NativePoint } from './native-scope.ts';
import type { TargetRead } from './native-target.ts';
/** A document-local node capability belongs to one run and one exact native frame. */
export interface NativeSelection {
  /** Native topology and isolated evaluator. */ scope: NativeScope;
  /** Run-specific isolated registry key. */ key: string;
  /** Unforgeable node selection capability. */ token: string;
  /** Read-only assertion snapshot. */ read: TargetRead;
}
/** Only the exact connected selected node can answer an operation. */
const NODE = `const slot=globalThis[p.key];if(!slot||slot.token!==p.token||!slot.node.isConnected)throw Error('Workflow target document or node changed');const node=slot.node;`;
/** Explicit native DOM commands, independent of page-world overridden functions. */
const OPERATIONS: Record<string, string> = {
  point: `node.scrollIntoView({block:'center',inline:'center',behavior:'instant'});const r=node.getBoundingClientRect(),x=r.left+r.width/2,y=r.top+r.height/2;let hit=node.getRootNode().elementFromPoint(x,y);if(!hit||(hit!==node&&!node.contains(hit)))throw Error('Workflow target is covered');if(node.disabled)throw Error('Workflow target disabled');return {x,y};`,
  editable: `if(node.disabled||node.readOnly||!(node.isContentEditable||node instanceof HTMLTextAreaElement||node instanceof HTMLInputElement))throw Error('Workflow target is not editable');return {type:node.type||'',editable:node.isContentEditable};`,
  focused: `let active=document.activeElement;while(active?.shadowRoot?.activeElement)active=active.shadowRoot.activeElement;if(active!==node&&!node.contains(active))throw Error('Workflow input focus changed');return true;`,
  valid: `return true;`,
  fillValue: `if(!(node instanceof HTMLInputElement)||node.disabled||node.readOnly)throw Error('Workflow field is not editable');node.value=p.value;if(node.value!==p.value)throw Error('Workflow field rejected value');node.dispatchEvent(new Event('input',{bubbles:true,composed:true}));node.dispatchEvent(new Event('change',{bubbles:true}));return true;`,
  select: `if(!(node instanceof HTMLSelectElement)||node.disabled)throw Error('Workflow target is not an enabled select');const options=[...node.options].filter(o=>o.label===p.value);if(options.length!==1||options[0].disabled||options[0].parentElement?.disabled)throw Error('Workflow select option missing, disabled or ambiguous');for(const option of node.options)option.selected=option===options[0];node.dispatchEvent(new Event('input',{bubbles:true,composed:true}));node.dispatchEvent(new Event('change',{bubbles:true}));return node.value;`,
  file: `if(!(node instanceof HTMLInputElement)||node.type!=='file'||node.disabled||node.webkitdirectory)throw Error('Workflow target is not an enabled ordinary file input');return {multiple:node.multiple};`,
  upload: `if(!(node instanceof HTMLInputElement)||node.type!=='file'||node.disabled||node.webkitdirectory)throw Error('Workflow file input is unavailable or a directory chooser');if(p.files.length>1&&!node.multiple)throw Error('Workflow input accepts only one file');const transfer=new DataTransfer();for(const f of p.files){const raw=atob(f.base64),bytes=Uint8Array.from(raw,c=>c.charCodeAt(0));transfer.items.add(new File([bytes],f.name,{type:f.type,lastModified:f.lastModified}));}node.files=transfer.files;node.dispatchEvent(new Event('input',{bubbles:true,composed:true}));node.dispatchEvent(new Event('change',{bubbles:true}));return true;`,
  fileNames: `return [...node.files].map(f=>f.name);`,
};
/** Encode capability and payload and reject unsupported operations before execution. */
export function targetScript(target: NativeSelection, operation: string, params: object = {}): string {
  if (!Object.hasOwn(OPERATIONS, operation)) throw Error('Unsupported native target operation');
  const payload = { ...params, key: target.key, token: target.token };
  return `(()=>{const p=${JSON.stringify(payload)};${NODE}${OPERATIONS[operation]}})()`;
}
/** Read through the exact native frame and document-scoped target, never a repeated selector. */
export function targetOperation<T>(target: NativeSelection, operation: string, params?: object): Promise<T> {
  return target.scope.evaluate<T>(targetScript(target, operation, params));
}
/** Hit-test locally and through the complete frame owner chain before pointer dispatch. */
export async function targetPoint(target: NativeSelection): Promise<NativePoint> {
  return target.scope.point(await targetOperation<NativePoint>(target, 'point'));
}
