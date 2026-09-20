// The same code on five browser vendors. The provider is one string; vendor keys live on your Oya key.
import { Oya, type Provider } from '@oya-ai/browser';

const oya = new Oya();
const vendors: Provider[] = ['oya-cloud', 'browserbase', 'steel', 'anchor', 'browseruse'];

const results = await Promise.allSettled(vendors.map(async (provider) => {
  await using browser = await oya.browser.start({ provider });
  await browser.goto('https://example.com');
  return (await browser.tabs()).find((t) => t.active)!.title;
}));
results.forEach((r, i) => console.log(vendors[i], r.status === 'fulfilled' ? r.value : r.reason.message));
