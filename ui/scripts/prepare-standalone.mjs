import { cp, mkdir } from 'node:fs/promises';

// Next's trace includes runtime dependencies; static assets are copied separately.
await mkdir('.next/standalone/.next', { recursive: true });
await cp('.next/static', '.next/standalone/.next/static', { recursive: true });
await cp('public', '.next/standalone/public', { recursive: true });
