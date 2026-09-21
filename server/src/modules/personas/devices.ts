/**
 * The device tables a fingerprint is drawn from. Their contents and order are
 * part of every existing persona's fingerprint: the seeded picks index into
 * them, so editing a seeded table moves devices under their cookie jars.
 */
import { CoreCount, MemoryGb } from './constants.ts';

// ── Realistic GPU databases by platform ──

/** Realistic GPUs by platform. */
export const GPU_DB = {
  Win32: [
    {
      vendor: 'Google Inc. (NVIDIA)',
      renderer: 'ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)',
      unmaskedVendor: 'NVIDIA Corporation',
      unmaskedRenderer: 'NVIDIA GeForce RTX 3060/PCIe/SSE2',
    },
    {
      vendor: 'Google Inc. (NVIDIA)',
      renderer: 'ANGLE (NVIDIA, NVIDIA GeForce RTX 3070 Direct3D11 vs_5_0 ps_5_0, D3D11)',
      unmaskedVendor: 'NVIDIA Corporation',
      unmaskedRenderer: 'NVIDIA GeForce RTX 3070/PCIe/SSE2',
    },
    {
      vendor: 'Google Inc. (NVIDIA)',
      renderer: 'ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 SUPER Direct3D11 vs_5_0 ps_5_0, D3D11)',
      unmaskedVendor: 'NVIDIA Corporation',
      unmaskedRenderer: 'NVIDIA GeForce GTX 1660 SUPER/PCIe/SSE2',
    },
    {
      vendor: 'Google Inc. (AMD)',
      renderer: 'ANGLE (AMD, AMD Radeon RX 6700 XT Direct3D11 vs_5_0 ps_5_0, D3D11)',
      unmaskedVendor: 'ATI Technologies Inc.',
      unmaskedRenderer: 'AMD Radeon RX 6700 XT',
    },
    {
      vendor: 'Google Inc. (Intel)',
      renderer: 'ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0, D3D11)',
      unmaskedVendor: 'Intel Inc.',
      unmaskedRenderer: 'Intel(R) UHD Graphics 630',
    },
  ],
  MacIntel: [
    {
      vendor: 'Google Inc. (Apple)',
      renderer: 'ANGLE (Apple, Apple M1, OpenGL 4.1)',
      unmaskedVendor: 'Apple',
      unmaskedRenderer: 'Apple M1',
    },
    {
      vendor: 'Google Inc. (Apple)',
      renderer: 'ANGLE (Apple, Apple M1 Pro, OpenGL 4.1)',
      unmaskedVendor: 'Apple',
      unmaskedRenderer: 'Apple M1 Pro',
    },
    {
      vendor: 'Google Inc. (Apple)',
      renderer: 'ANGLE (Apple, Apple M2, OpenGL 4.1)',
      unmaskedVendor: 'Apple',
      unmaskedRenderer: 'Apple M2',
    },
    {
      vendor: 'Google Inc. (Apple)',
      renderer: 'ANGLE (Apple, Apple M3, OpenGL 4.1)',
      unmaskedVendor: 'Apple',
      unmaskedRenderer: 'Apple M3',
    },
    {
      vendor: 'Google Inc. (Apple)',
      renderer: 'ANGLE (Apple, Apple M2 Pro, OpenGL 4.1)',
      unmaskedVendor: 'Apple',
      unmaskedRenderer: 'Apple M2 Pro',
    },
  ],
  'Linux x86_64': [
    {
      vendor: 'Google Inc. (NVIDIA)',
      renderer: 'ANGLE (NVIDIA, NVIDIA GeForce RTX 3080/PCIe/SSE2, OpenGL 4.5)',
      unmaskedVendor: 'NVIDIA Corporation',
      unmaskedRenderer: 'NVIDIA GeForce RTX 3080/PCIe/SSE2',
    },
    {
      vendor: 'Google Inc. (Intel)',
      renderer: 'ANGLE (Intel, Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6)',
      unmaskedVendor: 'Intel',
      unmaskedRenderer: 'Mesa Intel(R) UHD Graphics 630 (CFL GT2)',
    },
    {
      vendor: 'Google Inc. (AMD)',
      renderer: 'ANGLE (AMD, AMD Radeon RX 580, OpenGL 4.6)',
      unmaskedVendor: 'ATI Technologies Inc.',
      unmaskedRenderer: 'AMD Radeon RX 580',
    },
  ],
};

/** Common screen sizes and pixel ratios by platform. */
export const SCREEN_RESOLUTIONS = {
  Win32: [
    { width: 1920, height: 1080, dpr: 1 },
    { width: 2560, height: 1440, dpr: 1 },
    { width: 1366, height: 768, dpr: 1 },
    { width: 1680, height: 1050, dpr: 1 },
    { width: 3840, height: 2160, dpr: 1.5 },
  ],
  MacIntel: [
    { width: 1440, height: 900, dpr: 2 },
    { width: 1680, height: 1050, dpr: 2 },
    { width: 1920, height: 1080, dpr: 2 },
    { width: 2560, height: 1440, dpr: 2 },
    { width: 1280, height: 800, dpr: 2 },
  ],
  'Linux x86_64': [
    { width: 1920, height: 1080, dpr: 1 },
    { width: 2560, height: 1440, dpr: 1 },
    { width: 1366, height: 768, dpr: 1 },
  ],
};

/** Fonts a stock install of each platform has. */
export const FONT_SETS = {
  Win32: [
    'Arial',
    'Arial Black',
    'Calibri',
    'Cambria',
    'Comic Sans MS',
    'Consolas',
    'Courier New',
    'Georgia',
    'Impact',
    'Lucida Console',
    'Segoe UI',
    'Tahoma',
    'Times New Roman',
    'Trebuchet MS',
    'Verdana',
  ],
  MacIntel: [
    'Arial',
    'Arial Black',
    'Courier New',
    'Georgia',
    'Helvetica',
    'Helvetica Neue',
    'Impact',
    'Lucida Grande',
    'Menlo',
    'Monaco',
    'SF Pro',
    'Times New Roman',
    'Trebuchet MS',
    'Verdana',
  ],
  'Linux x86_64': [
    'Arial',
    'Courier New',
    'DejaVu Sans',
    'DejaVu Serif',
    'FreeMono',
    'FreeSans',
    'FreeSerif',
    'Liberation Mono',
    'Liberation Sans',
    'Liberation Serif',
    'Noto Sans',
    'Times New Roman',
    'Ubuntu',
    'Verdana',
  ],
};

/** Core counts a device may report, in pick order. */
export const HARDWARE_CONCURRENCY = [
  CoreCount.FOUR,
  CoreCount.SIX,
  CoreCount.EIGHT,
  CoreCount.TWELVE,
  CoreCount.SIXTEEN,
];
/** Device memory a device may report, in pick order. */
export const DEVICE_MEMORY = [MemoryGb.FOUR, MemoryGb.EIGHT, MemoryGb.SIXTEEN];

/** Timezones the seed picks from, by platform. */
export const TIMEZONES = {
  Win32: [
    'America/New_York',
    'America/Chicago',
    'America/Denver',
    'America/Los_Angeles',
    'America/Phoenix',
    'America/Detroit',
    'America/Indianapolis',
  ],
  MacIntel: [
    'America/New_York',
    'America/Chicago',
    'America/Denver',
    'America/Los_Angeles',
    'America/Phoenix',
    'Pacific/Honolulu',
  ],
  'Linux x86_64': [
    'America/New_York',
    'America/Chicago',
    'America/Los_Angeles',
    'Europe/London',
    'Europe/Berlin',
    'Asia/Tokyo',
    'UTC',
  ],
};

/** Locales the seed picks from, by platform; repeats weight the pick. */
export const LOCALES = {
  Win32: ['en-US', 'en-US', 'en-US', 'en-GB'],
  MacIntel: ['en-US', 'en-US', 'en-US', 'en-GB'],
  'Linux x86_64': ['en-US', 'en-US', 'en-GB', 'de-DE', 'ja-JP'],
};

/** What a persona may choose about its device. Everything else follows the seed. */
export const PLATFORMS = ['Win32', 'MacIntel', 'Linux x86_64'];

/**
 * Explicit choices, open to every platform: a Windows machine in Berlin is
 * ordinary. Wider than TIMEZONES/LOCALES above, which stay exactly as they
 * are, they drive the seeded pick, and changing them would move existing
 * fingerprints under their cookie jars.
 */
export const TIMEZONE_CHOICES = [
  'America/New_York',
  'America/Chicago',
  'America/Denver',
  'America/Los_Angeles',
  'America/Phoenix',
  'America/Detroit',
  'America/Indianapolis',
  'America/Anchorage',
  'Pacific/Honolulu',
  'America/Toronto',
  'America/Vancouver',
  'America/Mexico_City',
  'America/Sao_Paulo',
  'America/Argentina/Buenos_Aires',
  'Europe/London',
  'Europe/Dublin',
  'Europe/Lisbon',
  'Europe/Paris',
  'Europe/Berlin',
  'Europe/Madrid',
  'Europe/Rome',
  'Europe/Amsterdam',
  'Europe/Brussels',
  'Europe/Zurich',
  'Europe/Vienna',
  'Europe/Stockholm',
  'Europe/Oslo',
  'Europe/Copenhagen',
  'Europe/Helsinki',
  'Europe/Warsaw',
  'Europe/Prague',
  'Europe/Athens',
  'Europe/Istanbul',
  'Africa/Johannesburg',
  'Africa/Lagos',
  'Africa/Cairo',
  'Asia/Dubai',
  'Asia/Kolkata',
  'Asia/Singapore',
  'Asia/Hong_Kong',
  'Asia/Shanghai',
  'Asia/Tokyo',
  'Asia/Seoul',
  'Asia/Jakarta',
  'Asia/Manila',
  'Australia/Sydney',
  'Australia/Melbourne',
  'Pacific/Auckland',
  'UTC',
];
/** Locales a persona may choose explicitly, on any platform. */
export const LOCALE_CHOICES = [
  'en-US',
  'en-GB',
  'en-CA',
  'en-AU',
  'en-IN',
  'de-DE',
  'de-AT',
  'de-CH',
  'fr-FR',
  'fr-CA',
  'es-ES',
  'es-MX',
  'it-IT',
  'nl-NL',
  'pt-BR',
  'pt-PT',
  'sv-SE',
  'da-DK',
  'nb-NO',
  'fi-FI',
  'pl-PL',
  'cs-CZ',
  'tr-TR',
  'ru-RU',
  'ja-JP',
  'ko-KR',
  'zh-CN',
  'zh-TW',
  'hi-IN',
  'id-ID',
];
