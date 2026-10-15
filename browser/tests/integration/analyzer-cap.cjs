/** The analyzer's block cap keeps the part on screen, and its note and totals say what was kept. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

/**
 * A page of 3,200 rows: a link and a price text each, so 6,400 blocks, plus one email
 * field above the list, so 6,401 blocks; past the 5,000 cap.
 */
module.exports = async function analyzerCap(win, world, profile) {
  const view = { webContents: win.webContents };
  const rows = Array.from({ length: 3200 }, (_, i) => `<li><a href="/i/${i}">Item ${i}</a> price ${i}</li>`).join('');
  const list = path.join(profile, 'long-list.html');
  fs.writeFileSync(list, `<title>Long list</title><input type="email" aria-label="Top email"><ul>${rows}</ul>`);
  await win.loadFile(list);

  const top = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  assert.equal(top.totalBlocks, 6401, 'totalBlocks counts the blocks before the cap');
  assert.equal(top.blocks.length, 5000);
  assert.ok(
    top.blocks.some((b) => b.text === 'Item 0'),
    'the first item is on screen at scroll 0',
  );
  assert.match(
    top.facts.truncated,
    /around what is on screen/,
    'the first items are visible, so the cut is around them',
  );

  await win.webContents.executeJavaScript('window.scrollTo(0, document.documentElement.scrollHeight)');
  const bottom = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  assert.equal(bottom.truncated, true);
  assert.equal(bottom.totalBlocks, 6401);
  assert.equal(bottom.blocks.length, 5000);
  assert.match(bottom.facts.truncated, /showing 5000 of 6401 blocks around what is on screen/);
  const ids = new Set(bottom.blocks.map((b) => b.id).filter(Boolean));
  for (const element of bottom.elements.filter((e) => e.visible))
    assert.ok(ids.has(element.id), `visible element ${element.text} is kept`);
  assert.ok(
    bottom.blocks.some((b) => b.text === 'Item 3199'),
    'the last item, on screen at the bottom, is kept',
  );
  // The field is far above the screen, but a form field outranks the rest of the page.
  assert.ok(
    bottom.blocks.some((b) => b.kind === 'input:email'),
    'the off-screen email field is kept',
  );
  const visibleItems = bottom.elements.filter((e) => e.visible && /^Item \d+$/.test(e.text));
  assert.ok(visibleItems.length > 0, 'some items are on screen at the bottom');
  for (const item of visibleItems) {
    const at = bottom.blocks.findIndex((b) => b.id === item.id);
    assert.equal(
      bottom.blocks[at + 1]?.text,
      `price ${item.text.slice('Item '.length)}`,
      `the price beside ${item.text} is kept`,
    );
  }
  // Kept rows stay in page order: the item numbers read up, not shuffled by tier.
  const numbers = bottom.blocks
    .filter((b) => /^Item \d+$/.test(b.text))
    .map((b) => Number(b.text.slice('Item '.length)));
  assert.ok(
    numbers.every((n, i) => i === 0 || n > numbers[i - 1]),
    'the kept items are in reading order',
  );

  // Nothing on screen: a page of text rows with no element, from the top, as before.
  const text = path.join(profile, 'long-text.html');
  const paragraphs = Array.from({ length: 6400 }, (_, i) => `<p>Text ${i}</p>`).join('');
  fs.writeFileSync(text, `<title>Long text</title>${paragraphs}`);
  await win.loadFile(text);
  const plain = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  assert.equal(plain.totalBlocks, 6400);
  assert.equal(plain.blocks.length, 5000);
  assert.ok(
    plain.blocks.some((b) => b.text === 'Text 0'),
    'the top of the page is kept',
  );
  assert.match(plain.facts.truncated, /from the top/, 'nothing is on screen, so the note says from the top');
  console.log('PASS: analyzer cap keeps the on-screen part, and its note and totalBlocks tell the truth');
};
