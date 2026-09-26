// scratch: call Oya MCP tools on one browser (deleted after use)
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
const [base, browserId, ...calls] = process.argv.slice(2);
const t = new StreamableHTTPClientTransport(new URL(`${base}/mcp/${browserId}`), { requestInit: { headers: { Authorization: `Bearer ${process.env.KEY}` } } });
const c = new Client({ name: 'login-test', version: '1' }); await c.connect(t);
for (const call of calls) {
  const i = call.indexOf('='); const name = i < 0 ? call : call.slice(0, i); const json = i < 0 ? '' : call.slice(i + 1);
  const r = await c.callTool({ name, arguments: json ? JSON.parse(json) : {} }, undefined, { timeout: 180000 });
  console.log(`--- ${name}\n` + r.content.map((x) => x.text ?? `[${x.type}]`).join('\n').slice(0, 2500));
}
await c.close();
