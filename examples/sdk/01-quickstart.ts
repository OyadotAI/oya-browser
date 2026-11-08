// Start a browser, open a page, drive it in plain English. Nothing to configure first.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await using browser = await oya.browser.start(); // the desktop app if it is open, else Oya Cloud; stopped when the script exits
await browser.goto('https://news.ycombinator.com');
console.log(await browser.ask('What are the top 3 stories and their points?'));
