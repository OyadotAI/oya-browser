/**
 * Unit tests for the person's downloads: a page's download is saved straight to
 * the Downloads folder under a free name and listed per profile, its progress and
 * end are recorded, and only a saved file that is still there can be opened.
 */
import { it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { mkdtempSync, rmSync, existsSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Downloads, type StoredDownload } from '../../../../src/main/downloads/index.ts';

let folder = '';
beforeEach(() => {
  folder = mkdtempSync(join(tmpdir(), 'oya-downloads-'));
  mock.timers.enable({ apis: ['setTimeout'] });
});
afterEach(() => {
  mock.timers.reset();
  rmSync(folder, { recursive: true, force: true });
});

/** A page's download: its address, suggested name and bytes, saved where it is told. */
class FakeItem extends EventEmitter {
  /** Where the browser was told to save it. */
  path = '';
  /** Bytes so far. */
  received = 0;
  /** The address it comes from. */
  readonly url: string;
  /** The name the site suggested. */
  readonly name: string;
  /** Its size. */
  readonly total: number;
  /** `url`, `name` and `total` describe the file. */
  constructor(url = 'https://files.test/report.pdf', name = 'report.pdf', total = 2048) {
    super();
    [this.url, this.name, this.total] = [url, name, total];
  }
  /** The address it comes from. */
  getURL = () => this.url;
  /** The name the site suggested. */
  getFilename = () => this.name;
  /** Bytes so far. */
  getReceivedBytes = () => this.received;
  /** Its size. */
  getTotalBytes = () => this.total;
  /** Records the save path. */
  setSavePath = (path: string) => void (this.path = path);
  /** Where it is saving: '' until a path is set or the person picks one in Save As. */
  getSavePath = () => this.path;
}

/** Downloads over a kept list, a session, and the shown profile `partition`. */
function setup(kept: StoredDownload[] | null = null) {
  let saved = kept;
  const opened: string[] = [];
  const persona = { partition: 'persist:oya-a', partitionName: () => persona.partition };
  const shell = {
    openPath: async (p: string) => void opened.push(p),
    showItemInFolder: (p: string) => opened.push(`folder:${p}`),
  };
  const electron = { app: { getPath: () => folder }, shell };
  const file = { read: () => saved, write: (v: StoredDownload[]) => void (saved = JSON.parse(JSON.stringify(v))) };
  const downloads = new Downloads({ electron, persona } as any, file);
  const session = new EventEmitter();
  downloads.mayAutoSave = () => true;
  downloads.install(session as any, 'persist:oya-a');
  const start = (item = new FakeItem()) => (session.emit('will-download', {}, item), item);
  return { downloads, start, opened, persona, saved: () => saved };
}

it('saves a download straight to the Downloads folder and lists it, newest first', () => {
  const { downloads, start } = setup();
  const item = start();
  assert.equal(item.path, join(folder, 'report.pdf'));
  start(new FakeItem('https://files.test/photo.png', 'photo.png'));
  assert.deepEqual(
    downloads.list().map((d) => [d.title, d.state]),
    [
      ['photo.png', 'progressing'],
      ['report.pdf', 'progressing'],
    ],
  );
});

it('never overwrites: a second file of the same name gets " (2)"', () => {
  const { start } = setup();
  start();
  assert.equal(start().path, join(folder, 'report (2).pdf'));
});

it('keeps a suggested name to a plain file name in the Downloads folder', () => {
  const { start } = setup();
  assert.equal(start(new FakeItem(undefined, '../../etc/passwd')).path, join(folder, '.._.._etc_passwd'));
});

it('records progress at most once per interval, and the end at once', () => {
  const { downloads, start, saved } = setup();
  const item = start();
  item.received = 1024;
  item.emit('updated', {}, 'progressing');
  assert.equal(saved()![0].received, 0, 'not yet written');
  mock.timers.tick(1000);
  assert.equal(saved()![0].received, 1024);
  item.received = 2048;
  item.emit('done', {}, 'completed');
  assert.deepEqual([downloads.list()[0].state, downloads.list()[0].received], ['completed', 2048]);
});

it('a cancelled or failed download says so', () => {
  const { downloads, start } = setup();
  start().emit('done', {}, 'cancelled');
  start(new FakeItem(undefined, 'b.zip')).emit('done', {}, 'interrupted');
  assert.deepEqual(
    downloads.list().map((d) => d.state),
    ['interrupted', 'cancelled'],
  );
});

it('lists only the shown profile, and never says where a file was saved', () => {
  const { downloads, start, persona } = setup();
  start();
  assert.equal('path' in downloads.list()[0], false);
  persona.partition = 'persist:oya-b';
  assert.deepEqual(downloads.list(), []);
});

it('opens or shows only a saved file that is still there', () => {
  const { downloads, start, opened } = setup();
  const item = start();
  const id = downloads.list()[0].id;
  assert.equal(downloads.open(id), false, 'still downloading');
  item.emit('done', {}, 'completed');
  assert.equal(downloads.open(id), true);
  assert.equal(downloads.reveal(id), true);
  assert.deepEqual(opened, [item.path, `folder:${item.path}`]);
  rmSync(item.path);
  assert.equal(downloads.open(id), false, 'moved or deleted');
});

it('removing and clearing take finished downloads off the list but leave their files', () => {
  const { downloads, start } = setup();
  const done = start();
  done.emit('done', {}, 'completed');
  start(new FakeItem(undefined, 'running.zip'));
  downloads.forget(downloads.list()[1].id);
  downloads.clear();
  assert.deepEqual(
    downloads.list().map((d) => d.title),
    ['running.zip'],
  );
  assert.equal(existsSync(done.path), true);
});

it('a download running when the app quit comes back as failed', () => {
  const kept = [
    {
      id: 'x',
      url: 'https://f.test/a',
      title: 'a',
      time: 1,
      state: 'progressing',
      received: 1,
      total: 2,
      path: '/x',
      partition: 'persist:oya-a',
    },
  ];
  assert.equal(setup(kept as StoredDownload[]).downloads.list()[0].state, 'interrupted');
});

it('keeps only the scheme of a data: address, which carries the whole file', () => {
  const { downloads, start } = setup();
  start(new FakeItem(`data:text/plain;base64,${'A'.repeat(1000)}`, 'a.txt'));
  assert.equal(downloads.list()[0].url, 'data:');
});

/** Downloads where every page is one an agent drives, so each download goes to the Save As dialog. */
function agentSetup() {
  const parts = setup();
  parts.downloads.mayAutoSave = () => false;
  return parts;
}

it('a download from a page an agent drives is listed once the person picks where, under the name they chose', () => {
  const { downloads, start } = agentSetup();
  const item = start();
  assert.deepEqual(downloads.list(), [], 'nothing while Save As is open');
  item.path = join(folder, 'Q3 report.pdf');
  item.emit('updated', {}, 'progressing');
  assert.equal(downloads.list().length, 1);
  assert.equal(downloads.list()[0].title, 'Q3 report.pdf');
  item.emit('done', {}, 'completed');
  assert.equal(downloads.list().length, 1, 'listed once');
  assert.equal(downloads.list()[0].state, 'completed');
});

it('a Save As the person cancels is never listed', () => {
  const { downloads, start } = agentSetup();
  start().emit('done', {}, 'cancelled');
  assert.deepEqual(downloads.list(), []);
});

it('progress before a path is chosen lists nothing', () => {
  const { downloads, start } = agentSetup();
  start().emit('updated', {}, 'progressing');
  assert.deepEqual(downloads.list(), []);
});

it('the chosen path is used as it is, never claimed or renamed', () => {
  const { downloads, start, opened } = agentSetup();
  const item = start();
  const chosen = join(folder, 'mine.pdf');
  let told = '';
  item.setSavePath = (path: string) => void (told = path);
  item.path = chosen;
  item.emit('done', {}, 'completed');
  assert.equal(told, '', 'Oya never sets the save path of a file the person placed');
  assert.equal(existsSync(chosen), false, 'and never creates a file for it');
  writeFileSync(chosen, 'x');
  assert.equal(downloads.reveal(downloads.list()[0].id), true);
  assert.deepEqual(opened, [`folder:${chosen}`]);
});

it('a tiny file that goes straight to done after the choice is listed as saved', () => {
  const { downloads, start } = agentSetup();
  const item = start();
  item.path = join(folder, 'tiny.txt');
  item.emit('done', {}, 'completed');
  assert.equal(downloads.list()[0].state, 'completed');
});

it('a chosen download that fails or is cancelled later says so', () => {
  const { downloads, start } = agentSetup();
  const cancelled = start();
  cancelled.path = join(folder, 'a.pdf');
  cancelled.emit('updated', {}, 'progressing');
  cancelled.emit('done', {}, 'cancelled');
  const failed = start(new FakeItem(undefined, 'b.zip'));
  failed.path = join(folder, 'b.zip');
  failed.emit('updated', {}, 'interrupted');
  failed.emit('done', {}, 'interrupted');
  assert.deepEqual(
    downloads.list().map((d) => [d.title, d.state]),
    [
      ['b.zip', 'interrupted'],
      ['a.pdf', 'cancelled'],
    ],
  );
});

it('a transient interruption keeps a download in progress: only its end says how it went', () => {
  const { downloads, start } = setup();
  const item = start();
  item.emit('updated', {}, 'interrupted');
  mock.timers.tick(1000);
  assert.equal(downloads.list()[0].state, 'progressing', 'a resumable pause is not a failure');
  item.emit('done', {}, 'completed');
  assert.equal(downloads.list()[0].state, 'completed');
});

it('a download names the page it came from before a download-only tab closes', () => {
  const { downloads } = setup();
  const session = new EventEmitter();
  const order: string[] = [];
  const page = { id: 'page' };
  downloads.listedFrom = (id, contents) => order.push(`listed:${(contents as any)?.id}:${!!id}`);
  downloads.started = () => order.push('started');
  downloads.install(session as any, 'persist:oya-a');
  session.emit('will-download', {}, new FakeItem(), page);
  assert.deepEqual(order, ['listed:page:true', 'started']);
});

it('a Save As download names its page only once a path is chosen, and never when cancelled', () => {
  const { downloads } = agentSetup();
  const session = new EventEmitter();
  const told: string[] = [];
  downloads.listedFrom = (id) => told.push(id);
  downloads.install(session as any, 'persist:oya-b');
  const cancelled = new FakeItem();
  session.emit('will-download', {}, cancelled, {});
  cancelled.emit('done', {}, 'cancelled');
  const chosen = new FakeItem(undefined, 'b.zip');
  session.emit('will-download', {}, chosen, {});
  chosen.emit('updated', {}, 'progressing');
  assert.equal(told.length, 0, 'nothing before a path is chosen');
  chosen.path = join(folder, 'b.zip');
  chosen.emit('updated', {}, 'progressing');
  chosen.emit('done', {}, 'completed');
  assert.equal(told.length, 1, 'once, when it is listed');
});
