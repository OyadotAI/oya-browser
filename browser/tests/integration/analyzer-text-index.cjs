/** Large-page analysis must preserve exact-text ambiguity without rescanning the DOM for every control. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

/** Count full-document text-index passes deterministically rather than asserting machine-specific timings. */
module.exports = async function textIndex(win, world, profile) {
  const file = path.join(profile, 'many-controls.html');
  const buttons = Array.from({ length: 500 }, (_, i) => `<button>Product ${i}</button>`).join('');
  fs.writeFileSync(
    file,
    `<h2 id="heading">Save</h2><section id="first"><button>Save</button></section><section id="second"><button>Save</button></section>${buttons}`,
  );
  await win.loadFile(file);
  const view = { webContents: win.webContents };
  await world.evaluate(
    view,
    `globalThis.indexPasses=0;const nativeTags=document.getElementsByTagName.bind(document);document.getElementsByTagName=function(tag){if(tag==='*')indexPasses++;return nativeTags(tag);}`,
  );
  const read = async () => {
    const result = await world.evaluate(view, 'indexPasses=0;analyzePage()');
    assert.equal(result.ok, true);
    assert.equal(await world.evaluate(view, 'indexPasses'), 1, 'one document index, not one pass per element');
    return result.data.elements;
  };
  const before = await read();
  assert.equal(before.filter((e) => /^Product /.test(e.text)).length, 500);
  const save = before.filter((e) => e.text === 'Save' && e.type === 'button');
  assert.equal(save.length, 2);
  assert.ok(save.every((e) => e.repeats === 'true'));
  assert.deepEqual(
    save.map((e) => e.scoped),
    ['[id="first"] button:text-is("Save")', '[id="second"] button:text-is("Save")'],
  );
  await win.webContents.executeJavaScript(
    'document.querySelector("#second").remove();document.querySelector("#heading").remove()',
  );
  const after = await read();
  assert.equal(
    after.find((e) => e.text === 'Save').repeats,
    undefined,
    'removed duplicates cannot survive in a stale index',
  );
  await win.webContents.executeJavaScript(
    'const heading=document.createElement("h2");heading.textContent="Save";document.body.append(heading)',
  );
  assert.equal(
    (await read()).find((e) => e.text === 'Save' && e.type === 'button').repeats,
    'true',
    'new duplicates are detected on the next analysis',
  );
  console.log('PASS: 500 controls use one text-index pass; exact scopes and DOM-update invalidation preserved');
};
