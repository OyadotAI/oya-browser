/** Behavioral extensions for the opt-in public-site audit; no protocol or browser provider dependency. */
const assert = require('node:assert/strict');
/** These cases share the same analyzer-id targeting and native input as the core live audit. */
module.exports = async function extraCases(qa) {
  await require('./internet-content-cases.cjs')(qa);
  await require('./internet-remaining-cases.cjs')(qa);
  const { check, nav, body, analyze, element, click, read, keyboard, mouse, view, world, until } = qa;
  await check('ab-variant', '/abtest', async () => {
    await nav('/abtest');
    await until(async () => /A\/B Test (Control|Variation)/.test(await body()));
    assert.ok(JSON.stringify(await analyze()).includes('A/B Test'));
  });
  await check('numeric-input', '/inputs', async () => {
    await nav('/inputs');
    const input = await element((e) => e.tag === 'input');
    await click(input);
    await keyboard.type(view, '123');
    assert.equal(await read(input, 'value'), '123');
    await keyboard.press(view, 'ArrowUp');
    await until(async () => (await read(input, 'value')) === '124');
  });
  await check('horizontal-slider', '/horizontal_slider', async () => {
    await nav('/horizontal_slider');
    const input = await element((e) => e.tag === 'input');
    await click(input);
    const before = await read(input, 'value');
    await keyboard.press(view, 'ArrowRight');
    await until(async () => (await read(input, 'value')) !== before);
  });
  await check('notification-message', '/notification_message', async () => {
    await nav('/notification_message');
    await click(await element((e) => /^Click here$/i.test(e.text)));
    await until(async () => /Action (successful|unsuccessful)/.test(await body()));
    await analyze();
  });
  await check('redirect-navigation', '/redirector', async () => {
    await nav('/redirector');
    await click(await element((e) => e.text === 'here'));
    await until(
      async () => view.webContents.getURL().endsWith('/status_codes') && (await body()).includes('Status Codes'),
    );
    assert.ok((await analyze()).url.endsWith('/status_codes'));
  });
  await check('http-error-page', '/status_codes', async () => {
    await nav('/status_codes');
    await click(await element((e) => e.text === '404'));
    await until(async () => (await body()).includes('404 status code'));
    assert.ok((await analyze()).url.endsWith('/status_codes/404'));
  });
  await check('dynamic-content-refresh', '/dynamic_content', async () => {
    await nav('/dynamic_content');
    const before = await body();
    await click(await element((e) => e.text === 'click here'));
    // A transient empty document is not a successfully refreshed page.
    await until(async () => {
      const current = await body();
      return current.includes('Dynamic Content') && current !== before;
    });
    assert.ok((await body()).includes('Dynamic Content'));
    await analyze();
  });
  await check('entry-ad-close', '/entry_ad', async () => {
    await nav('/entry_ad');
    await click(await element((e) => /^Close$/i.test(e.text)));
    await until(async () => !(await analyze()).modal);
  });
  await check('floating-menu-scroll', '/floating_menu', async () => {
    await nav('/floating_menu');
    await element((e) => e.text === 'Home');
    await mouse.scroll(view, 900, 500, 0, 900);
    await until(async () => (await analyze()).scroll.y > 0);
    assert.ok((await analyze()).elements.some((e) => e.text === 'Home' && e.visible));
  });
  await check('broken-images-observation', '/broken_images', async () => {
    await nav('/broken_images');
    await until(
      async () =>
        await world.evaluate(view, '[...document.images].filter(i=>i.complete && !i.naturalWidth).length >= 2'),
    );
    assert.ok((await body()).includes('Broken Images'));
    await analyze();
  });
  await check('frames-navigation', '/frames', async () => {
    await nav('/frames');
    await click(await element((e) => e.text === 'Nested Frames'));
    await until(async () => JSON.stringify(await analyze()).includes('MIDDLE'));
  });
};
