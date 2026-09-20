/**
 * Register a released browser image as a Daytona snapshot.
 *
 * Cloud browsers boot from a snapshot, not from an image tag, so shipping a new
 * browser build is only half the job — until the image is registered and
 * DAYTONA_SNAPSHOT points at it, every provisioned browser is still the old
 * build. This runs from the release pipeline so the two cannot drift.
 *
 * Usage: node publish-snapshot.js --image <ref> --name <snapshot>
 *
 * Idempotent: re-running for a version that already has a snapshot reports it
 * and exits 0, so a re-run of a release does not fail here.
 */

const args = Object.fromEntries(
  process.argv
    .slice(2)
    .reduce((pairs, arg, i, all) => (arg.startsWith('--') ? [...pairs, [arg.slice(2), all[i + 1]]] : pairs), []),
);

const image = args.image;
const name = args.name;
if (!image || !name) {
  console.error('Usage: node publish-snapshot.js --image <ref> --name <snapshot>');
  process.exit(2);
}

const apiKey = process.env.DAYTONA_API_KEY;
if (!apiKey) {
  console.error('DAYTONA_API_KEY is not set — cannot register the snapshot.');
  process.exit(1);
}

const { Daytona } = await import('@daytona/sdk');
const daytona = new Daytona({
  apiKey,
  ...(process.env.DAYTONA_API_URL ? { apiUrl: process.env.DAYTONA_API_URL } : {}),
  target: process.env.DAYTONA_TARGET || 'us',
});

// Daytona has no registry credentials on a snapshot, so it pulls anonymously.
// A private image fails here rather than at provision time, which is the whole
// point of doing it in the pipeline.
console.log(`[snapshot] creating "${name}" from ${image}`);
try {
  await daytona.snapshot.create({ name, image }, { onLogs: (line) => console.log(`[daytona] ${line}`) });
  console.log(`[snapshot] created: ${name}`);
} catch (err) {
  const msg = err?.message || String(err);
  if (/already exists|conflict/i.test(msg)) {
    console.log(`[snapshot] "${name}" already exists — leaving it as is`);
  } else {
    console.error(`[snapshot] failed: ${msg}`);
    process.exit(1);
  }
}

// The name the deploy must point DAYTONA_SNAPSHOT at.
console.log(`snapshot=${name}`);
