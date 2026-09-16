// Release deployment updates each platform independently from the shipped assets.
export const browserDownloads = [
  { platform: 'macOS', architecture: 'Intel + Apple Silicon', href: '/downloads/Oya.Browser-1.0.83-universal.dmg' },
  { platform: 'Windows', architecture: 'x64', href: '/downloads/Oya.Browser-1.0.83-x64.exe' },
  { platform: 'Linux', architecture: 'x64', href: '/downloads/Oya.Browser-1.0.83-x64.AppImage' },
] as const;
