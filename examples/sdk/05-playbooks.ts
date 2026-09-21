// Record a flow once with the agent, save it as a playbook, then replay it without the LLM.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await using browser = await oya.browser.start(); // stopped when the script exits, even on error

// 1. Ask once. The agent reads `data` and types it through {{placeholders}}; `secrets` it never sees.
await browser.goto('https://httpbin.org/forms/post');
console.log(await browser.ask(
  'Order a large pizza with bacon for {{name}}, phone {{phone}}, email {{email}}, then submit the order.',
  { data: { name: 'Ada Lovelace', phone: '555-0100', email: 'ada@example.com' } },
));

// 2. Freeze that run into a playbook. The placeholders became its variables.
const playbook = await browser.toPlaybook('pizza-order');
console.log(`${playbook.name}: ${playbook.steps} steps, variables ${playbook.variables.join(', ')}`);
console.log(playbook.code); // the same flow as a Playwright module

// 3. Replay with new data. No LLM, unless the page changed: then the agent finishes
//    the order and saves its fix as the draft "pizza-order:draft" (autoHeal: false throws instead).
const replay = await browser.play('pizza-order', { name: 'Alan Turing', phone: '555-0199', email: 'alan@example.com' });
console.log('replayed', replay);

// 4. Or submit it in the background and get called back.
const run = await browser.submit({ playbook: 'pizza-order' }, {
  data: { name: 'Grace Hopper', phone: '555-0142', email: 'grace@example.com' },
  onSuccess: (result) => console.log('order placed', result),
  onFailure: (error) => console.error('order failed:', error.message),
  onHumanAttention: async (request) => {
    console.log(`needs a person (${request.reason}): ${request.message}`, request.liveViewUrl ?? '');
    await request.respond('done'); // after handling it, or your answer when reason is 'agent'
  },
  onHealed: (result) => console.log(`the form changed; fix saved as ${result.draft}, promote it with oya.playbooks.promote()`),
});
await run.done.catch(() => {}); // onFailure already reported it
