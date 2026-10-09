/** Remaining public demos exercised through native Oya input, permissions, auth and download events. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { nativePointer } = require('../../src/main/input/index.ts');
const { POINTER_COMMANDS } = require('../../src/main/actions/pointer-commands.ts');
const { watchNativeDialogs } = require('../../src/main/native/index.ts');

/** Scope the demo's published admin/admin credentials to this exact test contents and host. */
async function authenticated(qa, operation) {
  const { app, view } = qa;
  // These demos share a host but use incompatible schemes; don't send cached Basic credentials to Digest.
  await view.webContents.session.clearAuthCache();
  const login = (event, contents, details, auth, reply) => {
    if (contents !== view.webContents || auth.isProxy || auth.host !== 'the-internet.herokuapp.com') return;
    event.preventDefault();
    reply('admin', 'admin');
  };
  app.on('login', login);
  try {
    await operation();
  } finally {
    app.off('login', login);
    await view.webContents.session.clearAuthCache();
  }
}

/** Save only the chosen demo text file to a fixed scratch path; never open or execute its contents. */
async function download(qa, route) {
  const { nav, element, click, view, until, dir } = qa;
  await nav(route);
  const link = await element((e) => e.text === 'some-file.txt');
  const dest = path.join(dir, route.replaceAll('/', '-') + '.txt');
  let state;
  let received;
  let item;
  const listener = (event, candidate, contents) => {
    if (contents !== view.webContents) return;
    if (candidate.getURL() !== link.href) {
      event.preventDefault();
      return;
    }
    item = candidate;
    candidate.setSavePath(dest);
    candidate.on('updated', () => {
      if (candidate.getReceivedBytes() > 1048576) candidate.cancel();
    });
    candidate.once('done', (_event, result) => {
      received = candidate.getReceivedBytes();
      state = result;
    });
  };
  view.webContents.session.on('will-download', listener);
  try {
    await click(link);
    await until(() => !!state);
    assert.equal(state, 'completed');
    assert.ok(fs.statSync(dest).size > 0);
    assert.equal(fs.statSync(dest).size, received);
  } finally {
    if (item && !state) item.cancel();
    view.webContents.session.off('will-download', listener);
  }
}

/** Do not mask absent native capabilities or site-side failures as passing page-load checks. */
module.exports = async function remainingCases(qa) {
  const { check, nav, body, analyze, element, click, read, view, world, until, mouse, keyboard, locate } = qa;
  for (const route of ['/basic_auth', '/digest_auth']) {
    await check(route.slice(1), route, () =>
      authenticated(qa, async () => {
        await nav(route);
        await until(async () => (await body()).includes('Congratulations!'));
        assert.ok(JSON.stringify(await analyze()).includes('Congratulations!'));
      }),
    );
  }
  await check('download', '/download', () => download(qa, '/download'));
  await check('authenticated-download', '/download_secure', () =>
    authenticated(qa, () => download(qa, '/download_secure')),
  );
  await check('context-menu-alert', '/context_menu', async () => {
    await nav('/context_menu');
    let message;
    const stop = watchNativeDialogs(
      view.webContents,
      (info, reply) => {
        message = info.messageText;
        reply(true);
      },
      () => {},
    );
    try {
      const pt = await locate(await element((e) => e.type === 'contextmenu'));
      await mouse.move(view, pt.x, pt.y);
      nativePointer(view, { type: 'mouseDown', button: 'right', clickCount: 1, x: pt.x, y: pt.y });
      nativePointer(view, { type: 'mouseUp', button: 'right', clickCount: 1, x: pt.x, y: pt.y });
      await until(() => !!message);
      assert.equal(message, 'You selected a context menu');
    } finally {
      stop();
    }
  });
  await check('html-drag-and-drop', '/drag_and_drop', async () => {
    await nav('/drag_and_drop');
    const a = await element((e) => e.type === 'draggable' && e.text === 'A');
    const b = await element((e) => e.type === 'draggable' && e.text === 'B');
    const from = await locate(a),
      to = await locate(b);
    qa.win.focus();
    view.webContents.focus();
    await until(() => qa.win.isFocused() && view.webContents.isFocused());
    const driver = {
      mouse,
      activeView: () => view,
      deps: { sendResult: (_id, ok, _data, error) => assert.equal(ok, true, error) },
    };
    await POINTER_COMMANDS.drag(driver, 'live-drag', { from_x: from.x, from_y: from.y, to_x: to.x, to_y: to.y });
    await until(async () => (await read(a, 'textContent')).trim() === 'B');
    assert.equal((await read(b, 'textContent')).trim(), 'A');
  });
  await check('exit-intent-modal', '/exit_intent', async () => {
    await nav('/exit_intent');
    await mouse.move(view, 400, 250);
    nativePointer(view, { type: 'mouseLeave', x: 400, y: -1 });
    await click(await element((e) => /^Close$/i.test(e.text)));
    await until(async () => !(await analyze()).modal);
  });
  await check('hover-reveals-profile', '/hovers', async () => {
    await nav('/hovers');
    const point = await world.evaluate(
      view,
      `(()=>{const i=[...document.images].find(i=>i.alt==='User Avatar');const r=i.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`,
    );
    await mouse.move(view, point.x, point.y);
    const link = await element((e) => e.text === 'View profile');
    assert.ok(link.href.endsWith('/users/1'));
    assert.ok((await body()).includes('name: user1'));
    await mouse.move(view, 900, 500);
    await until(async () => !(await analyze()).elements.some((e) => e.text === 'View profile'));
  });
  await check('nested-menu-navigation', '/jqueryui/menu', async () => {
    await nav('/jqueryui/menu');
    const pt = await locate(await element((e) => e.text === 'Enabled'));
    await mouse.move(view, pt.x, pt.y);
    await click(await element((e) => e.text === 'Back to JQuery UI'));
    await until(async () => view.webContents.getURL().endsWith('/jqueryui'));
    await analyze();
  });
  await check('javascript-error-isolation', '/javascript_error', async () => {
    const errors = [];
    const listener = (event) => {
      if (event.level === 'error' || event.level === 3) errors.push(event.message);
    };
    view.webContents.on('console-message', listener);
    try {
      await nav('/javascript_error');
      await until(() => errors.some((e) => /not defined|undefined|TypeError|ReferenceError/i.test(e)));
      assert.ok(JSON.stringify(await analyze()).includes('JavaScript error'));
      await nav('/checkboxes');
      const box = await element((e) => e.type === 'checkbox');
      const before = await read(box, 'checked');
      await click(box);
      await until(async () => (await read(box, 'checked')) !== before);
    } finally {
      view.webContents.off('console-message', listener);
    }
  });
  await check('sortable-table', '/tables', async () => {
    await nav('/tables');
    const rows = () =>
      world.evaluate(
        view,
        '[...document.getElementsByTagName("table")[0].tBodies[0].rows].map(r=>r.cells[0].textContent)',
      );
    const before = await rows();
    await click(await element((e) => e.text === 'Last Name'));
    await until(async () => JSON.stringify(await rows()) !== JSON.stringify(before));
    assert.deepEqual(await rows(), [...before].sort());
  });
  await check('geolocation-denied-without-disclosure', '/geolocation', async () => {
    let denied = false;
    view.webContents.session.setPermissionRequestHandler((contents, permission, reply) => {
      if (contents === view.webContents && permission === 'geolocation') denied = true;
      reply(false);
    });
    try {
      await nav('/geolocation');
      await click(await element((e) => e.text === 'Where am I?'));
      await until(() => denied);
      assert.ok(!(await body()).includes('Latitude:'));
    } finally {
      view.webContents.session.setPermissionRequestHandler(null);
    }
  });
  await check('forgot-password-submit', '/forgot_password', async () => {
    await nav('/forgot_password');
    await click(await element((e) => e.tag === 'input' && e.name === 'email'));
    await keyboard.type(view, 'oya-native-audit@example.invalid');
    await click(await element((e) => e.text === 'Retrieve password'));
    await until(async () => {
      const text = await body();
      if (/Internal Server Error/i.test(text))
        throw Error('Site failure: forgot-password submission returned Internal Server Error');
      return /e-mail.*sent|email.*sent/i.test(text);
    });
  });
  await check('native-file-upload', '/upload', async () => {
    await nav('/upload');
    await element((e) => e.inputType === 'file');
    const filename = 'oya-native-audit.txt';
    const file = path.join(qa.dir, filename);
    fs.writeFileSync(file, 'Generated Oya native upload fixture. No user files.\n');
    let answered = false;
    const handler = (event, info, reply) => {
      event.preventDefault();
      if (
        info.url !== view.webContents.getURL() ||
        info.processId !== view.webContents.mainFrame.processId ||
        info.routingId !== view.webContents.mainFrame.routingId ||
        info.multiple
      )
        return reply([]);
      answered = true;
      reply([file]);
    };
    view.webContents.on('-oya-file-chooser', handler);
    try {
      const input = await element((e) => e.inputType === 'file');
      await click(input);
      await until(() => answered);
      await until(async () => (await read(input, 'files.length')) === 1);
      await click(await element((e) => /^Upload$/i.test(e.text)));
      await until(async () => (await body()).includes('File Uploaded!'));
      assert.ok((await body()).includes(filename));
    } finally {
      view.webContents.off('-oya-file-chooser', handler);
    }
  });
  await check('rich-text-editing', '/tinymce', async () => {
    await nav('/tinymce');
    await until(async () => {
      const page = await analyze();
      if (JSON.stringify(page).includes('no more editor loads available'))
        throw Error('Site failure: TinyMCE is read-only because the site has exhausted its monthly editor quota');
      return page.elements.some((e) => e.type === 'editable');
    });
    const editor = await element((e) => e.type === 'editable');
    await click(editor);
    await keyboard.type(view, 'Oya native rich text');
    await until(async () => JSON.stringify(await analyze()).includes('Oya native rich text'));
  });
};
