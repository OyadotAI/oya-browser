import { Oya } from '@oya-ai/browser';
const oya = new Oya();
const c = await oya.config.get<Record<string, unknown>>();
const secret = /key|secret|token|password/i;
for (const [k, v] of Object.entries(c)) {
  const shown = secret.test(k) ? (v ? '<set>' : '<empty>')
    : typeof v === 'string' ? (v.length > 40 ? '<long string>' : v)
    : v && typeof v === 'object' ? (Array.isArray(v) ? `[${v.length}]` : `{${Object.keys(v).join(',')}}`) : v;
  console.log(k.padEnd(28), shown);
}
console.log('--- personas:', JSON.stringify((await oya.personas.list()).map((p) => [p.id, p.name])));
console.log('--- running browsers:', (await oya.browser.list()).length);
