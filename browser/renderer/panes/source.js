/**
 * The Source pane: the page as the agent reads it and as HTML, refreshed on
 * demand, from View Page Source, or from an element the person inspected. The
 * read is shown in the format picked in the pane (markdown, TOON or JSONL),
 * which starts at the default chosen in settings; switching renders the same
 * analysis again, without reading the page again. It says which page it
 * read, marks the read stale when the page changes, shows errors where the
 * read would be, and never lets a slow switch overwrite a newer one.
 */
/* global oyaBrowser, Dom, DevPanel, RendererConstants */
/* exported SourcePane */

/** What each format is called above the read. */
const FORMAT_LABELS = { markdown: 'Markdown', toon: 'TOON', jsonl: 'JSONL' };

/** What the pane says before anything is read. */
const SOURCE_PROMPT = 'The current page, as the agent reads it. Refresh to read it.';

/** What the read half says before anything is read. */
const READ_PROMPT = 'Refresh, or right-click the page and choose View Page Source.';

/** The source pane. */
const SourcePane = {
  /** The page source last loaded: its HTML, its read, the analysis behind the read, and the page. */
  cached: { html: '', markdown: '', analysis: null, url: '' },
  /** Bumped by every format switch, so only the latest one's answer is shown. */
  generation: 0,

  /** Shows `text` in one half, or the placeholder when there is none, and whether it can be copied. */
  fill(id, text, placeholder, copy) {
    Dom.byId(id).textContent = text || placeholder;
    Dom.byId(id).classList.toggle('empty', !text);
    Dom.byId(copy).disabled = !text;
  },

  /** Shows the cached source, or the error or notice where the read would be. */
  render() {
    const { markdown, html, error, notice } = SourcePane.cached;
    const read = error ? 'Could not read the page: ' + error : markdown;
    SourcePane.fill('source-markdown', read, notice || 'Nothing to read on this page', 'source-copy-page');
    Dom.byId('source-markdown').classList.toggle('error', !!error);
    SourcePane.fill('source-html', html, 'No HTML', 'source-copy-html');
  },

  /** Says which page the read is of. */
  where(url) {
    Dom.byId('source-url').textContent = url ? 'Read from ' + url : SOURCE_PROMPT;
    Dom.byId('source-url').classList.remove('stale');
  },

  /** The page changed since it was read: say so, and that Refresh reads it again. */
  pageChanged() {
    if (!SourcePane.cached.url) return;
    Dom.byId('source-url').textContent = 'The page changed. Refresh to read it again.';
    Dom.byId('source-url').classList.add('stale');
  },

  /** The format picked in the pane. */
  format() {
    return [...document.querySelectorAll('input[name="source-format"]')].find((r) => r.checked)?.value || 'markdown';
  },

  /** Picks `format` in the pane without rendering. */
  pick(format) {
    const radio = document.querySelector(`input[name="source-format"][value="${format}"]`);
    if (radio) radio.checked = true;
    Dom.byId('source-page-label').textContent = FORMAT_LABELS[SourcePane.format()];
  },

  /** Shows the cached read in the picked format, rendering it again when it was written in another. */
  async show() {
    const generation = ++SourcePane.generation;
    const format = SourcePane.format();
    Dom.byId('source-page-label').textContent = FORMAT_LABELS[format];
    const { analysis } = SourcePane.cached;
    const page = analysis && analysis.format !== format ? await SourcePane.rerender(analysis, format) : undefined;
    if (generation !== SourcePane.generation) return;
    if (page !== undefined) SourcePane.keep(page, analysis, format);
    SourcePane.render();
  },

  /** Keeps a read rendered again in `format`. */
  keep(page, analysis, format) {
    SourcePane.cached = { ...SourcePane.cached, markdown: page, analysis: { ...analysis, format } };
  },

  /** The kept analysis written in `format`; a failed render is said in place of the read. */
  async rerender(analysis, format) {
    try {
      return await oyaBrowser.renderPage(analysis, format);
    } catch (error) {
      return 'Could not show this format: ' + error.message;
    }
  },

  /** Loads the current page's source, with Refresh off until it is back. */
  async refresh() {
    const btn = Dom.byId('source-refresh');
    btn.disabled = true;
    btn.textContent = 'Reading…';
    await oyaBrowser.getPageSource().then(SourcePane.loaded, (error) => SourcePane.loaded({ error: error.message }));
    btn.disabled = false;
    btn.textContent = 'Refresh';
  },

  /** Keeps and shows the page's source, or its error or notice. */
  loaded(source) {
    const { html = '', markdown = '', analysis = null, url = '', error, notice } = source || {};
    SourcePane.cached = { html, markdown, analysis, url, error, notice };
    SourcePane.where(url);
    return SourcePane.show();
  },

  /** Forgets the source and shows the prompt again. */
  clear() {
    SourcePane.cached = { html: '', markdown: '', analysis: null, url: '' };
    SourcePane.where('');
    SourcePane.fill('source-markdown', '', READ_PROMPT, 'source-copy-page');
    SourcePane.fill('source-html', '', 'No HTML yet.', 'source-copy-html');
  },

  /** View Page Source: show the pane with the page's source. */
  viewSource(data) {
    DevPanel.show('source');
    return SourcePane.loaded(data);
  },

  /** An inspected element: its read (or the error) alone; the page's HTML is not the element's. */
  inspected(result) {
    DevPanel.show('source');
    const ok = result?.ok && result.data?.page;
    const error = ok ? undefined : result?.error || 'Unknown error';
    return SourcePane.loaded({ markdown: ok ? result.data.page : '', analysis: ok ? result.data : null, error });
  },

  /** Copies one half and says so on its button. */
  async copy(button, source) {
    try {
      await navigator.clipboard.writeText(Dom.byId(source).textContent);
      button.textContent = 'Copied';
    } catch {
      button.textContent = 'Copy failed';
    }
    setTimeout(() => (button.textContent = 'Copy'), RendererConstants.COPIED_MS);
  },

  /** The default format chosen in settings: the pane starts at it, and follows it when it changes. */
  setDefault(format) {
    if (!Object.hasOwn(FORMAT_LABELS, format)) return;
    Dom.byId('page-format-preference').value = format;
    SourcePane.pick(format);
  },

  /** The person picked a default format in settings: save it, and show the read in it. */
  chooseDefault() {
    const format = Dom.byId('page-format-preference').value;
    oyaBrowser.saveUiPreferences({ pageFormat: format });
    SourcePane.setDefault(format);
    return SourcePane.show();
  },
};

Dom.byId('source-refresh').addEventListener('click', SourcePane.refresh);
Dom.byId('pane-source').addEventListener('change', (e) => e.target.name === 'source-format' && SourcePane.show());
Dom.byId('page-format-preference').addEventListener('change', SourcePane.chooseDefault);
Dom.byId('source-copy-page').addEventListener('click', (e) => SourcePane.copy(e.target, 'source-markdown'));
Dom.byId('source-copy-html').addEventListener('click', (e) => SourcePane.copy(e.target, 'source-html'));
oyaBrowser.getUiPreferences().then((value) => SourcePane.setDefault(value.pageFormat));
oyaBrowser.onViewSource(SourcePane.viewSource);
oyaBrowser.onInspectResult(SourcePane.inspected);
oyaBrowser.onUrlChanged(SourcePane.pageChanged);
