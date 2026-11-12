/** Exercise the opt-in setup renderer with real prompt readers and isolated synthetic terminal events. */
import assert from 'node:assert/strict';
import { stdin, stdout } from 'node:process';
import { spawnSync } from 'node:child_process';

const originalOutputTty = Object.getOwnPropertyDescriptor(stdout, 'isTTY');
Object.defineProperty(stdout, 'isTTY', { value: true, configurable: true });
delete process.env.NO_COLOR;
process.env.TERM = 'xterm-256color';
const { choose, ask, askSecret, closePrompts } = await import('../../src/prompt.ts');
const { presentQuestion } = await import('../../src/prompt/presentation.ts');
const { cellWidth } = await import('../../src/prompt/layout.ts');
const ESC = '\u001b';
const ANSI = new RegExp(`${ESC}\\[[0-9;?]*[A-Za-z]`, 'g');
const HELP = {
  title: 'Target',
  purpose: 'Choose your infrastructure.',
  find: 'Choose one of the options.',
  behavior: 'Enter accepts the highlighted option.',
};
const OPTIONS = [
  { id: 'docker', label: 'Docker', note: 'recommended; change settings later in oya.env' },
  { id: 'k8s', label: 'Kubernetes', note: 'Existing cluster · browser pods' },
];

/** Own geometry, echo capture and input methods only for this synthetic terminal. */
async function terminal(columns, rows, keys, run) {
  const properties = ['isTTY', 'isRaw', 'columns', 'rows'];
  const savedInput = Object.fromEntries(properties.map((key) => [key, Object.getOwnPropertyDescriptor(stdin, key)]));
  const savedOutput = Object.fromEntries(properties.map((key) => [key, Object.getOwnPropertyDescriptor(stdout, key)]));
  const methods = { setRawMode: stdin.setRawMode, resume: stdin.resume, write: stdout.write };
  let output = '';
  Object.defineProperty(stdin, 'isTTY', { value: true, configurable: true });
  Object.defineProperty(stdin, 'isRaw', { value: false, writable: true, configurable: true });
  for (const [key, value] of Object.entries({ columns, rows }))
    Object.defineProperty(stdout, key, { value, writable: true, configurable: true });
  stdin.setRawMode = (on) => {
    stdin.isRaw = on;
    return stdin;
  };
  stdin.resume = () => stdin;
  stdout.write = (chunk) => {
    output += String(chunk);
    return true;
  };
  try {
    const answer = run();
    for (const key of keys) {
      await new Promise((resolve) => setImmediate(resolve));
      if (typeof key === 'function') key();
      else stdin.emit('data', Buffer.from(key));
    }
    return { value: await answer, output };
  } finally {
    stdin.setRawMode = methods.setRawMode;
    stdin.resume = methods.resume;
    stdout.write = methods.write;
    for (const [stream, saved] of [
      [stdin, savedInput],
      [stdout, savedOutput],
    ])
      for (const key of properties) {
        if (saved[key]) Object.defineProperty(stream, key, saved[key]);
        else delete stream[key];
      }
  }
}

for (const columns of [80, 40]) {
  const result = await terminal(columns, 30, [`${ESC}[B`, '\r'], () =>
    choose('Target', OPTIONS, presentQuestion(HELP)),
  );
  assert.equal(result.value, 'k8s');
  const initial = result.output.split(new RegExp(`${ESC}\\[[0-9]+A`))[0].replace(ANSI, '');
  for (const line of initial.split('\n')) assert.ok(cellWidth(line) <= columns, `overflows ${columns}: ${line}`);
  if (columns === 40) {
    const draw = result.output.split(new RegExp(`${ESC}\\[[0-9]+A`))[0];
    const fullRows = draw.split('\n').filter((line) => cellWidth(line.replace(ANSI, '')) === columns);
    assert.ok(fullRows.length, 'exercise a menu description occupying the final column');
    for (const line of fullRows)
      assert.ok(!line.endsWith(`${ESC}[K`), 'clearing after a full row erases its final glyph');
  }
  assert.match(result.output.replace(ANSI, ''), /✓ Target\s+Kubernetes/);
}
console.log('✔ setup menu selection and geometry at 40/80 columns');

await assert.rejects(
  terminal(80, 30, ['\u0003'], () => choose('Target', OPTIONS, presentQuestion(HELP))),
  /Cancelled/,
);
console.log('✔ raw-menu cancellation restores input');
await assert.rejects(
  terminal(
    80,
    30,
    [
      () => {
        stdout.columns = 40;
        stdout.rows = 8;
      },
      '\u0003',
    ],
    () => choose('Target', OPTIONS, presentQuestion(HELP)),
  ),
  /Cancelled/,
);
console.log('✔ cancellation still wins immediately after a terminal resize');

const resized = await terminal(
  80,
  30,
  [
    () => {
      stdout.columns = 40;
      stdout.rows = 8;
    },
    `${ESC}[B`,
    '2\r',
  ],
  () => choose('Target', OPTIONS, presentQuestion(HELP)),
);
assert.equal(resized.value, 'k8s');
assert.match(resized.output, /2\. Kubernetes/);
console.log('✔ resize falls back to visible numbered selection');

const secret = await terminal(80, 40, ['wrong\r', 'SENTINEL-DO-NOT-PRINT\r'], () =>
  askSecret('Credentials', {
    presentation: presentQuestion({ ...HELP, title: 'Database credentials' }),
    validate: (value) => (value === 'SENTINEL-DO-NOT-PRINT' ? undefined : 'Use the complete credential.'),
  }),
);
assert.equal(secret.value, 'SENTINEL-DO-NOT-PRINT');
assert.doesNotMatch(secret.output, /SENTINEL-DO-NOT-PRINT|wrong/);
assert.match(secret.output, /Provided/);
assert.match(secret.output, /Use the complete credential/);
console.log('✔ secret validation and completion never expose entered values');
await assert.rejects(
  terminal(80, 40, ['alpha\r', '\u0003'], async () => {
    await ask('Name', '', { presentation: presentQuestion({ ...HELP, title: 'Name' }) });
    return choose('Target', OPTIONS, presentQuestion(HELP));
  }),
  /Cancelled/,
);
console.log('✔ text input then raw-menu cancellation releases the shared reader');

let lastResolved = false;
const sequenced = await terminal(
  80,
  40,
  ['alpha\r', `${ESC}[B`, '\r', () => assert.equal(lastResolved, false), 'omega\r'],
  async () => {
    const first = await ask('Name', '', { presentation: presentQuestion({ ...HELP, title: 'Name' }) });
    const choice = await choose('Target', OPTIONS, presentQuestion(HELP));
    const last = await ask('Label', 'fallback', { presentation: presentQuestion({ ...HELP, title: 'Label' }) });
    lastResolved = true;
    return { first, choice, last };
  },
);
assert.deepEqual(sequenced.value, { first: 'alpha', choice: 'k8s', last: 'omega' });
assert.doesNotMatch(sequenced.output, /SENTINEL/);
console.log('✔ raw-menu keys do not become phantom answers to the next text question');
closePrompts();

const plainScript = `const { choose, closePrompts } = await import('./src/prompt.ts'); const { presentQuestion } = await import('./src/prompt/presentation.ts'); try { console.log('PICKED:' + await choose('Target', ${JSON.stringify(OPTIONS)}, presentQuestion(${JSON.stringify(HELP)}))); } catch (err) { console.error(err.message); process.exitCode = 1; } finally { closePrompts(); }`;
for (const input of ['2\n', '']) {
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', plainScript], {
    cwd: new URL('../..', import.meta.url).pathname,
    env: { ...process.env, NO_COLOR: '1' },
    input,
    encoding: 'utf8',
    timeout: 5000,
  });
  assert.doesNotMatch(result.stdout + result.stderr, new RegExp(ESC));
  assert.equal(result.status, input ? 0 : 1);
  if (input) assert.match(result.stdout, /PICKED:k8s/);
  else assert.match(result.stderr, /Input ended/);
}
console.log('✔ plain output has no ANSI and EOF cannot accept setup defaults');
const plainTtyScript = `Object.defineProperty(process.stdin,'isTTY',{value:true});Object.defineProperty(process.stdout,'isTTY',{value:true});process.stdin.setRawMode=()=>process.stdin;const {ask,closePrompts}=await import('./src/prompt.ts');const {presentQuestion}=await import('./src/prompt/presentation.ts');try { console.log('VALUE:'+await ask('Domain','localhost',{presentation:presentQuestion(${JSON.stringify(HELP)})})); }catch(err){console.error(err.message);process.exitCode=1;}finally{closePrompts();}`;
for (const [input, expected] of [
  ['example.com\n', 'example.com'],
  ['exampleX\u007f.com\n', 'example.com'],
  ['example.com\u001b[D\u001b[DZ\n', 'example.cZom'],
  ['路径X\u007f.com\n', '路径.com'],
  ['\u0003', null],
]) {
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', plainTtyScript], {
    cwd: new URL('../..', import.meta.url).pathname,
    env: { ...process.env, NO_COLOR: '1' },
    input,
    encoding: 'utf8',
    timeout: 5000,
  });
  assert.doesNotMatch(result.stdout + result.stderr, new RegExp(ESC));
  assert.equal(result.status, expected === null ? 1 : 0);
  if (expected === null) {
    assert.match(result.stderr, /Cancelled/);
    assert.doesNotMatch(result.stdout, /VALUE:/);
  } else assert.ok(result.stdout.includes(`VALUE:${expected}`), result.stdout);
}
console.log('✔ NO_COLOR editing preserves values, emits no ANSI and cancels safely');
const redirectedScript = plainTtyScript.replace("Object.defineProperty(process.stdout,'isTTY',{value:true});", '');
const redirected = spawnSync(process.execPath, ['--input-type=module', '-e', redirectedScript], {
  cwd: new URL('../..', import.meta.url).pathname,
  env: { ...process.env, NO_COLOR: '1' },
  input: 'example.com\u001b[D\u001b[DZ\n',
  encoding: 'utf8',
  timeout: 5000,
});
assert.equal(redirected.status, 0, redirected.stderr);
assert.ok(redirected.stdout.includes('VALUE:example.cZom'));
for (const control of [ESC, '\r', '\b'])
  assert.ok(!redirected.stdout.includes(control), 'redirected output must append plain text');
console.log('✔ redirected output with terminal input contains no cursor controls');

const plainSecret = `Object.defineProperty(process.stdin,'isTTY',{value:true});Object.defineProperty(process.stdout,'isTTY',{value:true});process.stdin.setRawMode=()=>process.stdin;const {askSecret,closePrompts}=await import('./src/prompt.ts');const {presentQuestion}=await import('./src/prompt/presentation.ts');try { const value=await askSecret('Credentials',{presentation:presentQuestion(${JSON.stringify({ ...HELP, title: 'Credentials' })}),validate:value=>value==='SENTINEL'?undefined:'Check the credential.'});console.log(value==='SENTINEL'?'MATCHED':'ERROR'); }finally{closePrompts();}`;
const masked = spawnSync(process.execPath, ['--input-type=module', '-e', plainSecret], {
  cwd: new URL('../..', import.meta.url).pathname,
  env: { ...process.env, NO_COLOR: '1' },
  input: 'bad\rSENTINELX\u007f\r',
  encoding: 'utf8',
  timeout: 5000,
});
assert.equal(masked.status, 0, masked.stderr);
assert.match(masked.stdout, /MATCHED/);
assert.doesNotMatch(masked.stdout, /SENTINEL|bad/);
assert.doesNotMatch(masked.stdout + masked.stderr, new RegExp(ESC));
console.log('✔ plain-mode secret editing stays masked across validation retries');
if (originalOutputTty) Object.defineProperty(stdout, 'isTTY', originalOutputTty);
else delete stdout.isTTY;
console.log('All setup presentation checks passed.');
