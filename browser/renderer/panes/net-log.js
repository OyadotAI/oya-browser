/**
 * The Activity pane: every message between this browser and the server, with
 * direction and type filters. One listener serves every row, and selecting
 * text in an open row leaves it open.
 */
/* global oyaBrowser, Dom, RendererConstants */
/* exported NetLog */

/** The activity log. */
const NetLog = {
  /** The active filter. */
  filter: 'all',

  /** Which entries each filter shows. */
  FILTERS: {
    all: () => true,
    in: (entry) => entry.dir === 'in',
    out: (entry) => entry.dir === 'out',
    cmd: (entry) => entry.type.startsWith('cmd:') || entry.type === 'auth',
    result: (entry) => entry.type.startsWith('result:'),
  },

  /** How each direction reads. */
  DIRECTIONS: { in: 'From server', out: 'To server' },

  /** Opens or closes the row that was clicked, unless the click ended a text selection. */
  toggleRow(event) {
    const row = event.target.closest('.dev-entry');
    if (row && !String(window.getSelection?.() || '')) row.classList.toggle('expanded');
  },

  /** Whether the active filter shows `entry` (an unknown filter shows everything). */
  shows(entry) {
    return !Object.hasOwn(NetLog.FILTERS, NetLog.filter) || NetLog.FILTERS[NetLog.filter](entry);
  },

  /** A timestamp as HH:MM:SS.mmm, local time. */
  fmtTime(ts) {
    const d = new Date(ts);
    const pad = (n, width = RendererConstants.CLOCK_DIGITS) => String(n).padStart(width, '0');
    return (
      [d.getHours(), d.getMinutes(), d.getSeconds()].map((n) => pad(n)).join(':') +
      '.' +
      pad(d.getMilliseconds(), RendererConstants.MS_DIGITS)
    );
  },

  /** One log row. */
  createLogEntry(entry) {
    const div = document.createElement('div');
    div.className = 'dev-entry';
    div.dataset.dir = entry.dir;
    div.dataset.type = entry.type;
    div.innerHTML = NetLog.entryHtml(entry);
    return div;
  },

  /** A row's markup: time, direction, type and data, escaped. */
  entryHtml(entry) {
    return `<div class="head">
        <span class="ts">${NetLog.fmtTime(entry.ts)}</span>
        <span class="dir ${entry.dir === 'in' ? 'in' : 'out'}">${NetLog.DIRECTIONS[entry.dir] || NetLog.DIRECTIONS.out}</span>
        <span class="msg-type">${Dom.esc(entry.type)}</span>
      </div>
      <div class="body">${Dom.esc(entry.data || '')}</div>`;
  },

  /** Adds an entry, following the bottom when already there, and trims the oldest. */
  add(raw) {
    const entry = { ...raw, type: String(raw?.type || 'message') };
    const netLog = Dom.byId('net-log');
    const netEntry = NetLog.createLogEntry(entry);
    Dom.byId('net-empty').hidden = true;
    netEntry.style.display = NetLog.shows(entry) ? '' : 'none';
    netLog.appendChild(netEntry);
    NetLog.follow(netLog);
    while (netLog.children.length > RendererConstants.NET_LOG_LIMIT) netLog.removeChild(netLog.firstChild);
  },

  /** Keeps the newest row in view when the log was already scrolled to the bottom. */
  follow(netLog) {
    const fromBottom = netLog.scrollHeight - netLog.scrollTop - netLog.clientHeight;
    if (fromBottom < RendererConstants.NET_LOG_STICK_PX) netLog.scrollTop = netLog.scrollHeight;
  },

  /** Switches the filter and re-filters what is already shown. */
  setFilter(filter) {
    NetLog.filter = filter;
    document.querySelectorAll('.net-filter').forEach((b) => b.classList.toggle('active', b.dataset.filter === filter));
    Dom.byId('net-log')
      .querySelectorAll('.dev-entry')
      .forEach((row) => (row.style.display = NetLog.shows(row.dataset) ? '' : 'none'));
  },

  /** Empties the log. */
  clear() {
    Dom.byId('net-log').innerHTML = '';
    Dom.byId('net-empty').hidden = false;
  },
};

oyaBrowser.onDevLog(NetLog.add);
Dom.byId('net-log').addEventListener('click', NetLog.toggleRow);
document.querySelectorAll('.net-filter').forEach((btn) => {
  btn.addEventListener('click', () => NetLog.setFilter(btn.dataset.filter));
});
