/** A self-contained development entry lets macOS reopen Oya without terminal-only arguments. */
import path from 'node:path';

/** Keep the diagnostic mode and test profile identical for direct launches and OS deep links. */
export function nativeDevelopmentEntry(browserDir: string, profile: string): string {
  return `/** Generated local Oya development entry; not a distributable release. */
const { app } = require('electron');
app.setAppPath(${JSON.stringify(path.resolve(browserDir))});
process.env.OYA_USER_DATA_DIR = ${JSON.stringify(path.resolve(profile))};
if (!process.argv.includes('--oya-native-browsing')) process.argv.push('--oya-native-browsing');
require(${JSON.stringify(path.resolve(browserDir, 'out/main/index.js'))});
`;
}
