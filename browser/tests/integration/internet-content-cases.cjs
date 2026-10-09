/** Live content, refresh and scrolling checks using Oya's production analyzer and native inputs. */
const assert = require('node:assert/strict');

/** Exercise dynamic documents without replacing site handlers or synthesizing DOM events. */
module.exports = async function contentCases(qa) {
  const { check, nav, body, analyze, element, click, world, view, until, mouse } = qa;
  await check('challenging-dom-refresh', '/challenging_dom', async () => {
    await nav('/challenging_dom');
    const button = await element((e) => e.tag === 'a' && ['foo', 'bar', 'baz', 'qux'].includes(e.text));
    const before = await world.evaluate(view, 'performance.timeOrigin');
    await click(button);
    await until(async () => (await world.evaluate(view, 'performance.timeOrigin')) !== before);
    await until(async () => (await body()).includes('Iuvaret0'));
    assert.ok(JSON.stringify(await analyze()).includes('Iuvaret0'));
  });
  await check('disappearing-elements-refresh', '/disappearing_elements', async () => {
    for (let attempt = 0; attempt < 3; attempt++) {
      await nav('/disappearing_elements');
      for (const label of ['Home', 'About', 'Contact Us', 'Portfolio'])
        assert.ok(
          (await analyze()).elements.some((e) => e.text === label),
          `Missing persistent link ${label}`,
        );
    }
    await click(await element((e) => e.text === 'Home'));
    await until(async () => (await body()).includes('Available Examples'));
  });
  await check('infinite-scroll-growth', '/infinite_scroll', async () => {
    await nav('/infinite_scroll');
    await until(async () => world.evaluate(view, 'document.documentElement.scrollHeight > innerHeight'));
    const before = (await body()).length;
    await mouse.scroll(view, 900, 500, 0, 2000);
    await until(async () => (await body()).length > before && (await analyze()).scroll.y > 0);
  });
  await check('large-dom-analysis', '/large', async () => {
    await nav('/large');
    await until(async () => (await body()).includes('50.50'));
    const result = JSON.stringify(await analyze());
    assert.ok(result.includes('Large & Deep DOM'));
    assert.ok(result.includes('50.50'), 'Analyzer lost the end of the large table');
  });
  await check('typos-preserve-site-content', '/typos', async () => {
    await nav('/typos');
    await until(async () => (await body()).includes('Sometimes you'));
    const text = await body();
    const variant = text.includes('won,t') ? 'won,t' : "won't";
    assert.ok(text.includes(variant));
    assert.ok(JSON.stringify(await analyze()).includes(variant), 'Analyzer must not silently correct site text');
  });
  await check('shifting-content-navigation', '/shifting_content', async () => {
    await nav('/shifting_content');
    await click(await element((e) => e.text.includes('Example 1: Menu Element')));
    await until(async () => (await body()).includes('Shifting Content: Menu Element'));
    const refresh = await element((e) => e.text === 'click here');
    const before = await world.evaluate(view, 'performance.timeOrigin');
    await click(refresh);
    await until(async () => (await world.evaluate(view, 'performance.timeOrigin')) !== before);
    await element((e) => e.text === 'Home');
  });
  await check('slow-resources-do-not-block-analysis', '/slow', async () => {
    await nav('/slow');
    await until(async () => (await body()).includes('Slow Resources'));
    assert.ok(JSON.stringify(await analyze()).includes('Slow Resources'));
  });
};
