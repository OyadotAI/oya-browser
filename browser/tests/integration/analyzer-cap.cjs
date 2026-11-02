/** The analyzer's block cap keeps the part on screen, and its note and totals say what was kept. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createRenderer } = require('../../src/page/render.ts');

/**
 * A row whose last text sits in a shadow root ("Price", then a badge's text) is on screen. Its span
 * is not one range in one document, so the analyzer must read it from the client rects, not the range box.
 */
async function shadowText(win, world, profile) {
  const view = { webContents: win.webContents };
  const file = path.join(profile, 'shadow-text.html');
  fs.writeFileSync(file, '<title>Shadow text</title><p>Price <my-badge></my-badge></p><p>Second line</p>');
  await win.loadFile(file);
  await win.webContents.executeJavaScript(
    "document.querySelector('my-badge').attachShadow({ mode: 'open' }).textContent = 'nine dollars'",
  );
  const data = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const row = data.blocks.find((b) => (b.text || '').includes('Price'));
  assert.ok(row, 'the price row is in the analysis');
  assert.equal(row.screen, true, 'a row whose last text is in a shadow root, at the top of the page, is on screen');
}

/**
 * A small fixed frame (a chat widget) is pinned like a fixed header: its text never anchors the cut,
 * while the page text it covers still does.
 */
async function fixedFrame(win, world, profile) {
  const view = { webContents: win.webContents };
  const file = path.join(profile, 'fixed-frame.html');
  const rows = Array.from({ length: 600 }, (_, i) => `<p>Para ${i}</p>`).join('');
  fs.writeFileSync(
    file,
    `<title>Fixed frame</title><div style="height:20000px">${rows}</div><iframe srcdoc="<p>Chat help</p>" style="position:fixed;right:0;bottom:0;width:300px;height:200px"></iframe>`,
  );
  await win.loadFile(file);
  await win.webContents.executeJavaScript(`new Promise((resolve) => {
    const frame = document.querySelector('iframe');
    if (frame.contentDocument?.readyState === 'complete') return resolve();
    frame.addEventListener('load', resolve, { once: true });
  })`);
  await win.webContents.executeJavaScript(
    'window.scrollTo(0, (document.documentElement.scrollHeight - innerHeight) / 2)',
  );
  const data = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const chat = data.blocks.find((b) => b.text === 'Chat help');
  assert.ok(chat, 'the chat frame text is in the analysis');
  assert.notEqual(chat.screen, true, 'text in a small fixed frame (a chat widget) does not anchor the cut');
  assert.ok(
    data.blocks.some((b) => /^Para \d+$/.test(b.text) && b.screen === true),
    'the page text on screen still anchors the cut',
  );
}

/** A row that is only a <time> is on screen like a text row: the time element is part of its span. */
async function timeRow(win, world, profile) {
  const view = { webContents: win.webContents };
  const file = path.join(profile, 'time-row.html');
  fs.writeFileSync(file, '<title>Time row</title><div><time datetime="2026-10-16">today</time></div><p>Para</p>');
  await win.loadFile(file);
  const data = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const row = data.blocks.find((b) => (b.text || '').includes('2026-10-16'));
  assert.ok(row, 'the time row is in the analysis');
  assert.equal(row.screen, true, 'a row made only of a <time> at the top of the page is on screen');
}
/** A row inside a scrolling panel is on screen only where the panel shows it: text past the panel's edge is clipped. */
async function clippedPanel(win, world, profile) {
  const view = { webContents: win.webContents };
  const file = path.join(profile, 'clipped-panel.html');
  const inner = Array.from({ length: 200 }, (_, i) => `<p style="margin:0;height:20px">Inner ${i}</p>`).join('');
  fs.writeFileSync(file, `<title>Clipped panel</title><div style="height:300px;overflow:auto">${inner}</div>`);
  await win.loadFile(file);
  const data = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const row = (text) => data.blocks.find((b) => b.text === text);
  assert.equal(row('Inner 0')?.screen, true, 'the first row of the panel is on screen');
  assert.ok(row('Inner 20'), 'the row past the panel edge is in the analysis');
  assert.notEqual(row('Inner 20').screen, true, 'a row past the panel edge, still in the viewport, is clipped');
  assert.ok(row('Inner 150'), 'the row far down the panel is in the analysis');
  assert.notEqual(row('Inner 150').screen, true, 'a row far down the panel is not on screen');
}

/** Text in a shadow slot is read in the order it is shown, so a row's span still runs from its first to its last node. */
async function slotOrder(win, world, profile) {
  const view = { webContents: win.webContents };
  const file = path.join(profile, 'slot-order.html');
  fs.writeFileSync(
    file,
    '<title>Slot order</title><p><x-pair><span slot="a">Alpha</span><span slot="b">Beta</span></x-pair></p>',
  );
  await win.loadFile(file);
  await win.webContents.executeJavaScript(`(() => {
    const root = document.querySelector('x-pair').attachShadow({ mode: 'open' });
    const b = document.createElement('slot');
    b.name = 'b';
    const a = document.createElement('slot');
    a.name = 'a';
    root.append(b, a);
  })()`);
  const data = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const row = data.blocks.find((b) => (b.text || '').includes('Beta'));
  assert.ok(row, 'the row with the slotted text is in the analysis');
  assert.equal(row.screen, true, 'a row whose slotted text is shown first in the flat tree is on screen');
}

/** A page whose overflow sits on body (CSS hands it to the viewport) still shows its rows in the middle of the page. */
async function bodyOverflow(win, world, profile) {
  const view = { webContents: win.webContents };
  const file = path.join(profile, 'body-overflow.html');
  const rows = Array.from({ length: 3000 }, (_, i) => `<p>Para ${i}</p>`).join('');
  fs.writeFileSync(
    file,
    `<title>Body overflow</title><style>html,body{height:100%;margin:0}body{overflow-x:hidden}</style>${rows}`,
  );
  await win.loadFile(file);
  await win.webContents.executeJavaScript(
    'window.scrollTo(0, (document.documentElement.scrollHeight - innerHeight) / 2)',
  );
  const data = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const onScreen = data.blocks.filter((b) => b.screen === true).map((b) => Number(/^Para (\d+)$/.exec(b.text)?.[1]));
  assert.ok(
    onScreen.some((n) => n > 1000),
    'a row from the middle of the page is on screen, though body clips it',
  );
}

/** A modal dialog is the analysis root: its own fixed box must not pin its rows, or the cut falls back to the top. */
async function modalRoot(win, world, profile) {
  const view = { webContents: win.webContents };
  const file = path.join(profile, 'modal-root.html');
  const rows = Array.from({ length: 3000 }, (_, i) => `<p>Para ${i}</p>`).join('');
  const choices = Array.from({ length: 30 }, (_, i) => `<p>Choice ${i}</p>`).join('');
  fs.writeFileSync(
    file,
    `<title>Modal root</title>${rows}<div role="dialog" aria-modal="true" aria-label="Pick" style="position:fixed;top:20%;left:30%;width:40%;height:40%;overflow:auto;background:#fff">${choices}</div>`,
  );
  await win.loadFile(file);
  const data = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const row = data.blocks.find((b) => b.text === 'Choice 0');
  assert.ok(row, 'the dialog choices are in the analysis');
  assert.equal(row.screen, true, 'a row inside the modal root, a small fixed box, is on screen');
}

/** The analysis root is a dialog with its own scrolling panel: a row below the panel's edge is clipped, even in the viewport. */
async function modalClip(win, world, profile) {
  const view = { webContents: win.webContents };
  const file = path.join(profile, 'modal-clip.html');
  const choices = Array.from({ length: 30 }, (_, i) => `<p style="margin:0;height:40px">Choice ${i}</p>`).join('');
  fs.writeFileSync(
    file,
    `<title>Modal clip</title><div role="dialog" aria-modal="true" aria-label="Pick" style="position:fixed;top:20%;left:30%;width:40%;height:40%;overflow:auto;background:#fff">${choices}</div>`,
  );
  await win.loadFile(file);
  const data = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const row = (text) => data.blocks.find((b) => b.text === text);
  assert.equal(row('Choice 0')?.screen, true, 'the first choice is on screen');
  assert.ok(row('Choice 12'), 'a choice below the dialog edge is in the analysis');
  assert.notEqual(row('Choice 12').screen, true, 'a choice below the dialog edge, still in the viewport, is clipped');
}

/** A same-origin iframe clips its rows to its own visible area, whatever its overflow style: a row below the frame edge is not on screen. */
async function frameClip(win, world, profile) {
  const view = { webContents: win.webContents };
  const file = path.join(profile, 'frame-clip.html');
  const frameRows = Array.from({ length: 30 }, (_, i) => `<p style="margin:0;height:40px">Frame ${i}</p>`).join('');
  fs.writeFileSync(
    file,
    `<title>Frame clip</title><iframe srcdoc='${frameRows}' style="display:block;width:600px;height:300px;border:0"></iframe><p style="margin:0;height:40px">Page after</p>`,
  );
  await win.loadFile(file);
  await win.webContents.executeJavaScript(`new Promise((resolve) => {
    const frame = document.querySelector('iframe');
    if (frame.contentDocument?.readyState === 'complete') return resolve();
    frame.addEventListener('load', resolve, { once: true });
  })`);
  const data = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const row = (text) => data.blocks.find((b) => b.text === text);
  assert.equal(row('Frame 0')?.screen, true, 'the first row of the frame is on screen');
  assert.ok(row('Frame 12'), 'a row below the frame edge is in the analysis');
  assert.notEqual(
    row('Frame 12').screen,
    true,
    'a row below the frame edge, still in the viewport, is clipped by the frame',
  );
  assert.equal(row('Page after')?.screen, true, 'a page row after the frame, on screen, stays on screen');
}

/** A sticky heading that is not yet stuck (it sits below its top offset) is part of the page flow, so it is on screen. */
async function stickyInFlow(win, world, profile) {
  const view = { webContents: win.webContents };
  const file = path.join(profile, 'sticky-in-flow.html');
  fs.writeFileSync(
    file,
    '<title>Sticky in flow</title><div style="height:200px"></div><div style="height:3000px"><h2 style="position:sticky;top:0">Section</h2><p>Body</p></div>',
  );
  await win.loadFile(file);
  const data = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const row = data.blocks.find((b) => b.text === 'Section');
  assert.ok(row, 'the heading is in the analysis');
  assert.equal(row.screen, true, 'a sticky heading that is not stuck yet is on screen');
}

const spanCases = {
  shadowText,
  fixedFrame,
  timeRow,
  clippedPanel,
  slotOrder,
  bodyOverflow,
  modalRoot,
  modalClip,
  frameClip,
  stickyInFlow,
};

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

  // Nothing on screen: a page of text rows with no element, from the top, as before. The spacer puts
  // the text below the first screen, since text on screen anchors the cut like an element does.
  const text = path.join(profile, 'long-text.html');
  const paragraphs = Array.from({ length: 6400 }, (_, i) => `<p>Text ${i}</p>`).join('');
  fs.writeFileSync(text, `<title>Long text</title><div style="height:200000px"></div>${paragraphs}`);
  await win.loadFile(text);
  const plain = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  assert.equal(plain.totalBlocks, 6400);
  assert.equal(plain.blocks.length, 5000);
  assert.ok(
    plain.blocks.some((b) => b.text === 'Text 0'),
    'the top of the page is kept',
  );
  assert.match(plain.facts.truncated, /from the top/, 'nothing is on screen, so the note says from the top');
  // A long article with a sticky header, read in the middle: the header's links are pinned, so they never anchor the cut.
  const article = path.join(profile, 'long-article.html');
  const navLinks = Array.from({ length: 20 }, (_, i) => `<a href="/nav/${i}">Nav ${i}</a>`).join(' ');
  const articleRows = Array.from({ length: 6000 }, (_, i) => `<p>Para ${i}</p>`).join('');
  fs.writeFileSync(
    article,
    `<title>Long article</title><header style="position:sticky;top:0;height:60px;background:#fff">${navLinks}</header>${articleRows}`,
  );
  await win.loadFile(article);
  await win.webContents.executeJavaScript(
    'window.scrollTo(0, (document.documentElement.scrollHeight - innerHeight) / 2)',
  );
  const middle = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  assert.ok(
    !middle.blocks.some((b) => b.screen && /^Nav /.test(b.text)),
    'a link in the pinned sticky header does not anchor the cut',
  );
  const screenParas = middle.blocks.filter((b) => b.screen).map((b) => Number(/^Para (\d+)$/.exec(b.text)?.[1]));
  assert.ok(
    screenParas.some((n) => n > 2000 && n < 4000),
    'a paragraph from the middle of the article is on screen',
  );
  assert.equal(middle.blocks.length, 5000);
  assert.match(middle.facts.truncated, /around what is on screen/);
  assert.ok(!middle.blocks.some((b) => b.text === 'Para 0'), 'the top of the article is left out');
  const firstScreen = middle.blocks.find((b) => b.screen);
  for (const format of ['markdown', 'toon']) {
    const text = createRenderer(format).forAgent(middle, 20000);
    assert.ok(text.includes(firstScreen.text), `${format} keeps the first on-screen row`);
    assert.ok(!text.includes('Para 0'), `${format} leaves out the top of the article`);
    assert.match(text, /around what is on screen/, `${format} says the cut is around the screen`);
  }

  // A results page with a filter sidebar beside the products: its brands sit far above the middle of the page, so the products on screen are kept.
  const results = path.join(profile, 'long-results.html');
  const brands = Array.from({ length: 400 }, (_, i) => `<label><input type="checkbox"> Brand ${i}</label>`).join('');
  const products = Array.from(
    { length: 3000 },
    (_, i) => `<div><a href="/p/${i}">Product ${i}</a> <span>$${i}</span></div>`,
  ).join('');
  fs.writeFileSync(
    results,
    `<title>Results</title><div style="display:flex"><aside style="width:250px">${brands}</aside><main>${products}</main></div>`,
  );
  await win.loadFile(results);
  await win.webContents.executeJavaScript(
    'window.scrollTo(0, (document.documentElement.scrollHeight - innerHeight) / 2)',
  );
  const shop = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const shownProducts = shop.blocks.filter((b) => b.screen && /^Product \d+$/.test(b.text));
  assert.ok(shownProducts.length > 0, 'some products are on screen in the middle of the results');
  const shopText = createRenderer('toon').forAgent(shop, 20000);
  for (const product of shownProducts) assert.ok(shopText.includes(product.text), `${product.text} is kept`);
  assert.ok(!shopText.includes('Brand '), 'the filter sidebar, far above the screen, is left out');

  // A full-screen app shell scrolls its own content: the middle of that content is what is on screen.
  const shell = path.join(profile, 'long-shell.html');
  fs.writeFileSync(
    shell,
    `<title>Shell</title><div id="shell" style="position:fixed;inset:0;overflow:auto">${articleRows}</div>`,
  );
  await win.loadFile(shell);
  await win.webContents.executeJavaScript(
    '(() => { const s = document.getElementById("shell"); s.scrollTop = (s.scrollHeight - s.clientHeight) / 2; })()',
  );
  const app = (await world.evaluate(view, 'analyzePage({highlight:false})')).data;
  const appParas = app.blocks.filter((b) => b.screen).map((b) => Number(/^Para (\d+)$/.exec(b.text)?.[1]));
  assert.ok(
    appParas.some((n) => n > 2000 && n < 4000),
    'a paragraph from the middle of the app shell is on screen',
  );
  assert.match(app.facts.truncated, /around what is on screen/);
  await shadowText(win, world, profile);
  await fixedFrame(win, world, profile);
  await timeRow(win, world, profile);
  await clippedPanel(win, world, profile);
  await slotOrder(win, world, profile);
  await bodyOverflow(win, world, profile);
  await modalRoot(win, world, profile);
  await modalClip(win, world, profile);
  await frameClip(win, world, profile);
  await stickyInFlow(win, world, profile);
  console.log('PASS: analyzer cap keeps the on-screen part, and its note and totalBlocks tell the truth');
};
// Each span case runs alone from a control script, so one failing case cannot hide the others.
module.exports.spanCases = spanCases;
