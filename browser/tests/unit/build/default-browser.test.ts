/** NSIS registration must compile safely and preserve quoted Windows paths and URL arguments. */
import { readFileSync } from 'node:fs';
import { it } from 'node:test';
import assert from 'node:assert/strict';
const script = readFileSync(new URL('../../../build/default-browser.nsh', import.meta.url), 'utf8');
it('uses valid NSIS escapes for embedded double quotes', () => {
  assert.ok(!script.includes('$"'), 'bare dollar-quote is an unknown NSIS variable, not an escape');
  assert.equal(script.split(String.raw`$\"`).length - 1, 8);
});
it('keeps paths with spaces and the incoming URL separately quoted', () => {
  const executable = '$\\"$INSTDIR\\${APP_EXECUTABLE_FILENAME}$\\"';
  assert.ok(script.includes(`'${executable},0'`));
  assert.ok(script.includes(`'${executable} ${String.raw`$\"%1$\"`}'`));
  assert.ok(script.includes(`'${executable}'`));
});
it('registers HTTP and HTTPS candidates without replacing user defaults', () => {
  assert.match(script, /"http" "OyaBrowserURL"/);
  assert.match(script, /"https" "OyaBrowserURL"/);
  assert.doesNotMatch(script, /^\s*(WriteReg\w+|DeleteReg\w+).*UserChoice/m);
});
