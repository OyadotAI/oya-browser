// Start a browser (or hand over the one already connected), open a page, drive it in plain English.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await using browser = await oya.browser.start(); // stopped when the script exits, even on error
await browser.goto('https://news.ycombinator.com');
console.log(await browser.ask('What are the top 3 stories and their points?'));
