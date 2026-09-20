/**
 * The Source pane: the page as the agent reads it and as HTML, refreshed on
 * demand, from View Page Source, or from an element the person inspected. The
 * read is shown in the format picked in the pane (markdown, TOON or JSONL),
 * which starts at the default chosen in settings; switching renders the same
 * analysis again, without reading the page again.
 */
/* global oyaBrowser, Dom, DevPanel */
/* exported SourcePane */

/** What each format is called above the read. */
const FORMAT_LABELS = { markdown: 'Markdown', toon: 'TOON', jsonl: 'JSONL' };

/** The source pane. */
const SourcePane = {
  /** The page source last loaded: its HTML, its read, and the analysis behind the read. */
  cached: { html: '', markdown: '', analysis: null },

  /** Shows `text` in one half, or the placeholder when there is none. */
  fill(el, text, placeholder) {
    el.textContent = text || placeholder;
    el.classList.toggle('empty', !text);
  },

  /** Shows the cached source. */
  render() {
    SourcePane.fill(Dom.byId('source-markdown'), SourcePane.cached.markdown, 'No page read available');
    SourcePane.fill(Dom.byId('source-html'), SourcePane.cached.html, 'No HTML available');
  },

  /** The format picked in the pane. */
  format() {
    return document.querySelector('input[name="source-format"]:checked')?.value || 'markdown';
  },

  /** Picks `format` in the pane without rendering. */
  pick(format) {
    const radio = document.querySelector(`input[name="source-format"][value="${format}"]`);
    if (radio) radio.checked = true;
    Dom.byId('source-page-label').textContent = FORMAT_LABELS[SourcePane.format()];
  },

  /** Shows the cached read in the picked format, rendering it again when it was written in another. */
  async show() {
    Dom.byId('source-page-label').textContent = FORMAT_LABELS[SourcePane.format()];
    const { analysis } = SourcePane.cached;
    if (analysis && analysis.format !== SourcePane.format()) await SourcePane.rerender(analysis);
    SourcePane.render();
  },

  /** Writes the kept analysis in the picked format. */
  async rerender(analysis) {
    const format = SourcePane.format();
    const page = await oyaBrowser.renderPage(analysis, format);
    SourcePane.cached = { ...SourcePane.cached, markdown: page, analysis: { ...analysis, format } };
  },

  /** Loads the current page's source. */
  async refresh() {
    const btn = Dom.byId('source-refresh');
    btn.textContent = '⟳ Loading...';
    await oyaBrowser.getPageSource().then(SourcePane.loaded, SourcePane.failed);
    btn.textContent = '⟳ Refresh';
  },

  /** Keeps and shows the page's source. */
  loaded(source) {
    SourcePane.cached = { html: source.html || '', markdown: source.markdown || '', analysis: source.analysis };
    return SourcePane.show();
  },

  /** The source could not be loaded: say why where the read would be. */
  failed(e) {
    Dom.byId('source-markdown').textContent = 'Error: ' + e.message;
    Dom.byId('source-markdown').classList.remove('empty');
  },

  /** Forgets the source and shows the hints again. */
  clear() {
    SourcePane.cached = { html: '', markdown: '', analysis: null };
    for (const id of ['source-markdown', 'source-html']) {
      Dom.byId(id).textContent = 'Right-click → View Page Source or click Refresh';
      Dom.byId(id).classList.add('empty');
    }
  },

  /** View Page Source: show the pane with the page's source. */
  viewSource(data) {
    DevPanel.show('source');
    return SourcePane.loaded(data);
  },

  /** An inspected element: show its read (or the error) beside the page HTML. */
  inspected(result) {
    DevPanel.show('source');
    const ok = result?.ok && result.data?.page;
    const markdown = ok ? result.data.page : 'Error: ' + (result?.error || 'Unknown error');
    SourcePane.cached = { html: SourcePane.cached?.html || '', markdown, analysis: ok ? result.data : null };
    return SourcePane.show();
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
oyaBrowser.getUiPreferences().then((value) => SourcePane.setDefault(value.pageFormat));
oyaBrowser.onViewSource(SourcePane.viewSource);
oyaBrowser.onInspectResult(SourcePane.inspected);
