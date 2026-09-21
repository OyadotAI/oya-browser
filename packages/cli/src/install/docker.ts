/**
 * The Docker target: the governed network and image when that fleet was
 * chosen, `docker compose up`, and waiting for the server to report ready.
 */
import { capture, run } from './shell.ts';
import type { Answers } from './types.ts';
import { READY_POLL_MS, READY_TIMEOUT_MS } from './constants.ts';

/** The internal bridge and image governed browsers need, and how to give the server Docker access. */
async function provisionGoverned(root: string): Promise<void> {
  // verifyRuntime() refuses anything but an internal bridge, so create it that way.
  const exists = await capture('docker', ['network', 'inspect', 'oya-browsers']);
  if (!exists)
    await run('docker', ['network', 'create', '--internal', '--driver', 'bridge', 'oya-browsers'], { cwd: root });
  console.log('  building the governed browser image…');
  await run('docker', ['build', '-t', 'oya-browser:local', 'browser'], { cwd: root });
  console.log('\n  Governed browsers need Docker daemon access from the server container.');
  console.log('  Add this to the server service in docker-compose.yml, then re-run `docker compose up -d`:');
  console.log('    volumes:\n      - /var/run/docker.sock:/var/run/docker.sock');
}

/** Brings the stack up, with as many browser workers as asked for. */
export async function provisionDocker(root: string, a: Answers): Promise<void> {
  if (a.fleet === 'oya-selfhosted') await provisionGoverned(root);
  const scale = a.fleet === 'docker-workers' ? ['--scale', `browser=${a.workers}`] : ['--scale', 'browser=0'];
  await run('docker', ['compose', 'up', '-d', '--build', ...scale], { cwd: root });
}

/** Whether /readyz answers OK right now. */
async function isReady(url: string): Promise<boolean> {
  try {
    return (await fetch(`${url}/readyz`)).ok;
  } catch {
    return false; // not up yet
  }
}

/** Polls until /readyz answers or the deadline passes. */
async function pollReady(url: string, deadline: number): Promise<boolean> {
  while (Date.now() < deadline) {
    if (await isReady(url)) return true;
    await new Promise((r) => setTimeout(r, READY_POLL_MS));
  }
  return false;
}

/** Polls /readyz until it answers, or throws with where to look. */
export async function waitReady(url: string, timeoutMs = READY_TIMEOUT_MS): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  process.stdout.write('  waiting for /readyz… ');
  if (await pollReady(url, deadline)) {
    console.log('ready');
    return;
  }
  console.log('timed out');
  throw new Error(`${url}/readyz did not become ready. Check logs with: docker compose logs -f server`);
}
