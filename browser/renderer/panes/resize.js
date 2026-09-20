/**
 * Dragging the workspace panel's edge to resize it. Widths are sent to the
 * main process at most once per animation frame.
 */
/* global oyaBrowser, Dom, RendererConstants */
/* exported PanelResize */

/** The panel's drag handle. */
const PanelResize = {
  /** A drag is in progress. */
  dragging: false,
  /** The latest width not yet sent. */
  pendingWidth: undefined,
  /** The frame that will send it. */
  frame: undefined,

  /** Sends the pending width, if any. */
  flush() {
    PanelResize.frame = null;
    if (PanelResize.pendingWidth !== undefined) oyaBrowser.resizeDevPanel(PanelResize.pendingWidth);
    PanelResize.pendingWidth = undefined;
  },

  /** Shows the drag state on the handle and the page. */
  grab(on) {
    Dom.byId('dev-panel-resize').classList.toggle('dragging', on);
    document.body.style.cursor = on ? 'col-resize' : '';
    document.body.style.userSelect = on ? 'none' : '';
  },

  /** A primary-button press starts a drag (not in a narrow window). */
  down(e) {
    if (window.innerWidth < RendererConstants.RESIZE_MIN_WINDOW || e.button !== 0) return;
    e.preventDefault();
    PanelResize.dragging = true;
    Dom.byId('dev-panel-resize').setPointerCapture(e.pointerId);
    PanelResize.grab(true);
  },

  /** The width for a pointer at `x`: within the panel's limits, leaving the page its minimum. */
  widthAt(x) {
    const { PANEL_MIN_WIDTH, PANEL_MAX_WIDTH, PAGE_MIN_WIDTH } = RendererConstants;
    const width = Math.min(window.innerWidth - x, PANEL_MAX_WIDTH, window.innerWidth - PAGE_MIN_WIDTH);
    return Math.round(Math.max(PANEL_MIN_WIDTH, width));
  },

  /** Follows the pointer, batching widths into animation frames. */
  move(e) {
    if (!PanelResize.dragging) return;
    PanelResize.pendingWidth = PanelResize.widthAt(e.clientX);
    if (!PanelResize.frame) PanelResize.frame = requestAnimationFrame(PanelResize.flush);
  },

  /** Ends a drag, sending the final width now. */
  finish() {
    if (!PanelResize.dragging) return;
    PanelResize.dragging = false;
    cancelAnimationFrame(PanelResize.frame);
    PanelResize.flush();
    PanelResize.grab(false);
  },
};

{
  const handle = Dom.byId('dev-panel-resize');
  handle.addEventListener('pointerdown', PanelResize.down);
  handle.addEventListener('pointermove', PanelResize.move);
  for (const type of ['pointerup', 'pointercancel', 'lostpointercapture'])
    handle.addEventListener(type, PanelResize.finish);
  window.addEventListener('blur', PanelResize.finish);
}
