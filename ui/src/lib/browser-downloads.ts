/**
 * The desktop browser's download links, one per platform. Release deployment
 * updates each platform independently from the shipped assets.
 */

/** Platform, architecture and file for each desktop download. */
export const browserDownloads = [
  { platform: 'macOS', architecture: 'Intel + Apple Silicon', href: '/downloads/Oya.Browser-1.0.83-universal.dmg' },
  { platform: 'Windows', architecture: 'x64', href: '/downloads/Oya.Browser-1.0.83-x64.exe' },
  { platform: 'Linux', architecture: 'x64', href: '/downloads/Oya.Browser-1.0.83-x64.AppImage' },
] as const;
