/** Trusted locator algorithms run in a native isolated world, never through page-controlled helpers. */
import { ACCESSIBILITY_SOURCE } from './native-accessibility-source.ts';
/** Normalized text and open-shadow traversal match the workflow locator vocabulary. */
const HELPERS = `
 const norm=value=>String(value??'').replace(/\\s+/g,' ').trim();
 const roots=[document];for(let i=0;i<roots.length;i++)for(const n of roots[i].querySelectorAll('*'))if(n.shadowRoot)roots.push(n.shadowRoot);
 const all=roots.flatMap(root=>[...root.querySelectorAll('*')]);
 const text=n=>/^(SCRIPT|STYLE|NOSCRIPT|HEAD)$/.test(n.tagName)?'':norm(n.matches('input[type=button],input[type=submit]')?n.value:n.textContent);
 const visible=n=>{const r=n.getBoundingClientRect();return r.width>0&&r.height>0&&getComputedStyle(n).visibility!=='hidden';};
 const labels=n=>{const by=n.getAttribute('aria-labelledby');if(by)return by.split(/\\s+/).map(id=>n.getRootNode().getElementById?.(id)?.textContent||'');const label=n.getAttribute('aria-label');return label!==null?[label]:[...(n.labels||[])].map(l=>l.textContent);};
 const name=new RegExp('^\\\\W*'+p.candidate.value.replace(/[.*+?^\u0024{}()|[\\]\\\\]/g,'\\\\\u0024&')+'\\\\W*$');
`;
/** Fixed strategy dispatch; candidate text is encoded as data, never evaluated as code. */
const MATCH = `
 const c=p.candidate, wanted=norm(c.value);
 const strategies={
  css:()=>roots.flatMap(root=>[...root.querySelectorAll(c.value)]),
  testId:()=>all.filter(n=>n.getAttribute('data-testid')===c.value),
  placeholder:()=>all.filter(n=>n.getAttribute('placeholder')===c.value),
  label:()=>all.filter(n=>labels(n).some(label=>norm(label)===wanted)),
  text:()=>all.filter(n=>text(n)===wanted&&![...n.children].some(child=>text(child)===wanted)),
  role:()=>all.filter(n=>oyaAccessibility.getRole(n)===c.role&&!oyaAccessibility.isInaccessible(n)&&name.test(oyaAccessibility.computeAccessibleName(n)))
 };
 if(!Object.hasOwn(strategies,c.kind))throw Error('Unknown native workflow locator');
 const nodes=[...new Set(strategies[c.kind]())].filter(n=>p.hidden||visible(n));
 if(nodes.length!==1)return {count:nodes.length};
 const node=nodes[0], link=node.closest('a');
 const recorded=(!p.el.tag||!!node.closest(p.el.tag))&&(!p.el.href||(!!link?.href&&new URL(link.href).origin===new URL(p.el.href,location.href).origin&&new URL(link.href).pathname===new URL(p.el.href,location.href).pathname));
 globalThis[p.key]={node,token:p.token};
 return {count:1,text:norm(node.textContent),value:node.value,recorded,editable:node.isContentEditable||node instanceof HTMLTextAreaElement||node instanceof HTMLInputElement};
`;
/** Each successful read installs one document-local exact node capability for its run. */
export function locatorScript(params: object): string {
  return `(()=>{const p=${JSON.stringify(params)};${ACCESSIBILITY_SOURCE}\n${HELPERS}\n${MATCH}})()`;
}
