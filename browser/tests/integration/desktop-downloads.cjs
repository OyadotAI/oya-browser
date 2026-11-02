/**
 * The toolbar's download chip in the real shell: a page's download lands in the
 * Downloads folder and the Downloads button says "Saved" with the file's name,
 * drawn inline in the toolbar (no new view over the page), never with the path.
 */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { app } = require('electron');

/** Read the shell renderer. */
const read = (win, source) => win.webContents.executeJavaScript(source);

/** Downloads a small file from a fresh tab and checks the chip, the file and the views. */
module.exports = async function downloads({ win, call, until, profile }) {
  const folder = path.join(profile, 'Downloads');
  fs.mkdirSync(folder, { recursive: true });
  app.setPath('downloads', folder);
  const source = path.join(profile, 'chip-report.txt');
  fs.writeFileSync(source, 'saved by the download chip test');
  const tab = await call(win, 'newTab', 'about:blank');
  const views = win.getBrowserViews().length;
  const page = await until(() => win.getBrowserViews().find((v) => v.webContents.getURL() === 'about:blank'));
  page.webContents.downloadURL(pathToFileURL(source).href);
  await until(() => read(win, 'document.querySelector(".download-chip-status")?.textContent === "Saved"'));
  assert.equal(await read(win, 'document.querySelector(".download-chip-stem").textContent'), 'chip-report');
  assert.equal(await read(win, 'document.querySelector(".download-chip-ext").textContent'), '.txt');
  assert.match(
    await read(win, 'document.querySelector("#downloads-open").getAttribute("aria-label")'),
    /^Saved chip-report\.txt/,
  );
  assert.equal(
    await read(win, 'document.querySelector(".download-chip [role=status]").textContent'),
    'Saved chip-report.txt',
  );
  assert.doesNotMatch(
    await read(win, 'document.querySelector(".download-chip").outerHTML'),
    new RegExp(folder.replace(/[/\\]/g, '.')),
  );
  assert.equal(fs.readFileSync(path.join(folder, 'chip-report.txt'), 'utf8'), 'saved by the download chip test');
  assert.ok(win.getBrowserViews().length <= views, 'the chip draws no view over the page (the blank tab may close)');
  await read(win, 'document.querySelector("#downloads-open").focus()');
  win.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'Escape' });
  await until(() => read(win, 'document.querySelector(".download-chip").classList.contains("phase-idle")'));
  await call(win, 'closeTab', tab).catch(() => {}); // a blank tab opened only to download closes itself
};
