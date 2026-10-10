/** Replace only cloud allocation with a local native Oya process; preserve enrollment and command ownership. */
import { WebSocket } from 'ws';
import { once } from 'node:events';
import { openNativeFixture } from './native-browser.mjs';

/** Install a hermetic worker at the cloud-runtime seam, with real native persona setup and shutdown. */
export async function nativeProvider() {
  const { WORKERS } = await import('../../src/drivers/sandbox/worker.ts');
  const original = WORKERS.docker;
  const instances = new Map();
  const counts = { created: 0, released: 0 };
  WORKERS.docker = {
    ...original,
    async create(_config, spec) {
      const fixture = await enroll(spec);
      const instance = {
        labels: spec.labels,
        state: 'running',
        async destroy() {
          instances.delete(spec.name);
          await fixture.close();
          counts.released++;
        },
      };
      instances.set(spec.name, instance);
      counts.created++;
      return { id: spec.name };
    },
    async find(_config, name) {
      return instances.get(name) || null;
    },
    async list(_config, owner) {
      return [...instances.values()].filter((instance) => instance.labels['oya-owner'] === owner);
    },
  };
  return {
    counts,
    async close() {
      await Promise.all([...instances.values()].map((instance) => instance.destroy()));
      WORKERS.docker = original;
    },
  };
}

/** Use the production sandbox enrollment contract, not a preconnected desktop that MCP merely borrows. */
async function enroll(spec) {
  const browser = await openNativeFixture();
  const socket = new WebSocket(spec.env.OYA_SERVER_URL);
  let queue = Promise.resolve();
  try {
    await new Promise((resolve, reject) => {
      const deadline = setTimeout(() => reject(Error('Native cloud enrollment timed out')), 15000);
      const fail = (error) => {
        clearTimeout(deadline);
        reject(error);
      };
      socket.on('message', (raw) => {
        queue = queue
          .then(async () => {
            const msg = JSON.parse(raw);
            if (msg.type === 'auth_ok') {
              await browser.preparePersona(msg);
              clearTimeout(deadline);
              resolve();
            }
            if (msg.type === 'ping') socket.send(JSON.stringify({ type: 'pong' }));
            if (msg.type === 'profile_capture')
              for (const reply of await browser.captureProfile(msg.id)) socket.send(JSON.stringify(reply));
            if (msg.type === 'cmd') {
              const result = await browser.send(msg.action, msg.params);
              socket.send(JSON.stringify({ ...result, type: 'cmd_result', id: msg.id }));
            }
          })
          .catch(fail);
      });
      socket.once('error', fail);
      socket.once('close', () => fail(Error('Native cloud control socket closed')));
      socket.once('open', () =>
        socket.send(
          JSON.stringify({
            type: 'auth',
            api_key: spec.env.OYA_API_KEY,
            browser_id: spec.env.OYA_BROWSER_ID,
            browser_name: spec.env.OYA_BROWSER_NAME,
            persona: spec.env.OYA_PERSONA,
            provider: spec.env.OYA_PROVIDER,
            cdp: false,
            profile_sync: true,
            actions: ['navigate', 'analyze', 'click', 'type', 'list_tabs'],
          }),
        ),
      );
    });
    return {
      async close() {
        const closed = socket.readyState === WebSocket.CLOSED ? Promise.resolve() : once(socket, 'close');
        socket.terminate();
        await closed;
        await queue;
        await browser.close();
      },
    };
  } catch (error) {
    socket.terminate();
    await browser.close();
    throw error;
  }
}
