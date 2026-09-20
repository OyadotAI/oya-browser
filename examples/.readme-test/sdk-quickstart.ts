import { Oya } from '@oya-ai/browser';

const oya = new Oya();                            // reads OYA_API_KEY
await using browser = await oya.browser.start(); // stopped when the block exits, even on error
await browser.goto('https://news.ycombinator.com');
console.log(await browser.ask('What are the top 3 stories?'));
