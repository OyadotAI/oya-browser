/**
 * The Daytona SDK client, which now lives with the Daytona runtime in
 * workers/daytona.ts. Re-exported here so existing importers keep working.
 */
export { client, isNotFound } from './workers/daytona.ts';
