/**
 * The navigation toolbar: the address bar, back, forward and reload, and the
 * page's address and title as the main process reports them.
 */
/* global oyaBrowser, Dom */

Dom.byId('url-bar').addEventListener('keydown', (e) => {
  if (e.key !== 'Enter') return;
  oyaBrowser.navigate(Dom.byId('url-bar').value.trim());
  Dom.byId('url-bar').blur();
});

Dom.byId('btn-back').addEventListener('click', () => oyaBrowser.goBack());
Dom.byId('btn-forward').addEventListener('click', () => oyaBrowser.goForward());
Dom.byId('btn-reload').addEventListener('click', () => oyaBrowser.reload());

oyaBrowser.onUrlChanged((url) => {
  Dom.byId('url-bar').value = url;
});
oyaBrowser.onTitleChanged((title) => {
  document.title = title ? `${title}, Oya Browser` : 'Oya Browser';
});
