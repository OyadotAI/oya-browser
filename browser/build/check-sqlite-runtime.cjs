/** Verify that the packaged desktop's own Node runtime includes the SQLite features used by import. */
const assert = require('node:assert/strict');
assert.ok(process.versions.electron, 'Run this probe with the packaged Oya executable in Node mode.');
const { DatabaseSync } = require('node:sqlite');
const db = new DatabaseSync(':memory:', { allowExtension: false });
try {
  db.exec('CREATE TABLE fixture(time INTEGER); INSERT INTO fixture VALUES (13411699200000000)');
  const query = db.prepare('SELECT time FROM fixture');
  query.setReadBigInts(true);
  assert.equal(query.get().time, 13411699200000000n);
  process.stdout.write('PASS packaged desktop SQLite and 64-bit timestamps.\n');
} finally {
  db.close();
}
