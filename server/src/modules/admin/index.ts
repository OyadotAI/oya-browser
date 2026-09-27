/**
 * The admin module's facade: the routes, the download counter the downloads
 * route feeds, what the ping receiver records, and whether a license is revoked.
 */
export { router } from './routes.ts';
export { countDownload, drainDownloads } from './download-counter.ts';
export { recordPing, type Ping } from './repository.ts';
export { isRevoked } from './service.ts';
export { isAdmin } from './access.ts';
