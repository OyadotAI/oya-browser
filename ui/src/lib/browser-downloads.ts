/**
 * The desktop browser's download links, one per platform. They use the server's
 * version-free /download routes, which redirect to the newest installer, so a
 * release never has to rewrite them (pinned names went stale and 404ed).
 */

/** Platform, architecture and file for each desktop download. */
export const browserDownloads = [
  { platform: 'macOS', architecture: 'Intel + Apple Silicon', href: '/download/mac' },
  { platform: 'Windows', architecture: 'x64', href: '/download/windows' },
] as const;
