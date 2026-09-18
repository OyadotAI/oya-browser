/**
 * The Source pane: the page as Markdown and as HTML, refreshed on demand, from
 * View Page Source, or from an element the person inspected.
 */
/* global oyaBrowser, Dom, DevPanel */
/* exported SourcePane */

/** The source pane. */
const SourcePane = {
  /** The page source last loaded. */
  cached: { html: '', markdown: '' },

  /** Shows `text` in one half, or the placeholder when there is none. */
  fill(el, text, placeholder) {
    el.textContent = text || placeholder;
    el.classList.toggle('empty', !text);
  },

  /** Shows the cached source. */
  render() {
    SourcePane.fill(Dom.byId('source-markdown'), SourcePane.cached.markdown, 'No markdown available');
    SourcePane.fill(Dom.byId('source-html'), SourcePane.cached.html, 'No HTML available');
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
    SourcePane.cached = source;
    SourcePane.render();
  },

  /** The source could not be loaded: say why where the markdown would be. */
  failed(e) {
    Dom.byId('source-markdown').textContent = 'Error: ' + e.message;
    Dom.byId('source-markdown').classList.remove('empty');
  },

  /** Forgets the source and shows the hints again. */
  clear() {
    SourcePane.cached = { html: '', markdown: '' };
    for (const id of ['source-markdown', 'source-html']) {
      Dom.byId(id).textContent = 'Right-click → View Page Source or click Refresh';
      Dom.byId(id).classList.add('empty');
    }
  },

  /** View Page Source: show the pane with the page's source. */
  viewSource(data) {
    DevPanel.show('source');
    SourcePane.cached = { html: data.html || '', markdown: data.markdown || '' };
    SourcePane.render();
  },

  /** An inspected element: show its Markdown (or the error) beside the page HTML. */
  inspected(result) {
    DevPanel.show('source');
    const markdown =
      result?.ok && result.data?.markdown ? result.data.markdown : 'Error: ' + (result?.error || 'Unknown error');
    SourcePane.cached = { html: SourcePane.cached?.html || '', markdown };
    SourcePane.render();
  },
};

Dom.byId('source-refresh').addEventListener('click', SourcePane.refresh);
oyaBrowser.onViewSource(SourcePane.viewSource);
oyaBrowser.onInspectResult(SourcePane.inspected);
