const fs = require('node:fs');
const path = require('node:path');
const { spawn, execFileSync } = require('node:child_process');

// macOS uses the executable bundle's identity for the application menu, even
// when app.setName() and the menu template have the correct product name.
// Keep the installed Electron runtime untouched and cache a branded dev copy.
function developmentExecutable() {
  const electron = require('electron');
  if (process.platform !== 'darwin') return electron;
  const source = path.resolve(electron, '../../..');
  const version = require('electron/package.json').version;
  const cache = path.join(__dirname, 'node_modules', '.cache', 'oya-runtime', `${version}-v1`);
  const bundle = path.join(cache, 'Oya Browser.app');
  const ready = path.join(cache, 'ready');
  if (!fs.existsSync(ready)) {
    fs.mkdirSync(cache, { recursive: true });
    const staging = path.join(cache, `staging-${process.pid}.app`);
    try {
      fs.cpSync(source, staging, { recursive: true, verbatimSymlinks: true });
      const plist = path.join(staging, 'Contents', 'Info.plist');
      for (const [key, value] of Object.entries({ CFBundleName: 'Oya Browser', CFBundleDisplayName: 'Oya Browser', CFBundleIdentifier: 'ai.oya.browser.development' })) {
        execFileSync('/usr/libexec/PlistBuddy', ['-c', `Set :${key} ${value}`, plist]);
      }
      fs.copyFileSync(path.join(__dirname, 'build', 'icon.icns'), path.join(staging, 'Contents', 'Resources', 'electron.icns'));
      execFileSync('/usr/bin/codesign', ['--force', '--deep', '--sign', '-', staging], { stdio: 'pipe' });
      if (!fs.existsSync(bundle)) fs.renameSync(staging, bundle);
      fs.writeFileSync(ready, 'Oya Browser\n');
    } finally {
      if (fs.existsSync(staging)) fs.rmSync(staging, { recursive: true, force: true });
    }
  }
  return path.join(bundle, 'Contents', 'MacOS', 'Electron');
}

if (require.main === module) {
  const executable = developmentExecutable();
  if (process.argv.includes('--prepare')) {
    process.stdout.write(executable + '\n');
  } else {
    const child = spawn(executable, [__dirname, ...process.argv.slice(2)], { stdio: 'inherit' });
    child.on('error', error => { console.error(error.message); process.exitCode = 1; });
    child.on('exit', (code, signal) => { if (signal) process.kill(process.pid, signal); else process.exitCode = code || 0; });
    for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
  }
}
module.exports = { developmentExecutable };
