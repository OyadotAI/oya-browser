/**
 * The shell's layout as the main process computes it (panel width and reveal,
 * chrome height), and the resize handle's accessible name and keyboard.
 */
/* global oyaBrowser, Dom, RendererConstants */
/* exported ShellLayout */

/** Layout from the main process. */
const ShellLayout = {
  /** Applies a layout: CSS variables, the panel's open state, and the handle's range. */
  apply(layout) {
    const root = document.documentElement;
    root.style.setProperty('--panel-reveal', layout.reveal + 'px');
    Dom.byId('dev-panel').classList.toggle('open', layout.progress > 0);
    root.dataset.panelMoving = String(layout.progress > 0 && layout.progress < 1);
    root.style.setProperty('--panel-width', layout.panelWidth + 'px');
    root.style.setProperty('--panel-height', layout.panelHeight + 'px');
    root.style.setProperty('--chrome-height', layout.chromeHeight + 'px');
    ShellLayout.range(layout.panelWidth);
  },

  /** The handle's current value and maximum. */
  range(width) {
    const handle = document.querySelector('.dev-panel-resize');
    handle.setAttribute('aria-valuenow', width);
    handle.setAttribute(
      'aria-valuemax',
      Math.min(RendererConstants.PANEL_MAX_WIDTH, innerWidth - RendererConstants.PAGE_MIN_WIDTH),
    );
  },

  /** Makes the handle a focusable, labelled separator. */
  label(handle) {
    handle.tabIndex = 0;
    handle.setAttribute('role', 'separator');
    handle.setAttribute('aria-label', 'Resize workspace tools');
    handle.setAttribute('aria-orientation', 'vertical');
    handle.setAttribute('aria-valuemin', String(RendererConstants.PANEL_MIN_WIDTH));
  },

  /** The width an arrow, Home or End key asks for. */
  keyWidth(key) {
    const { PANEL_MIN_WIDTH, PANEL_MAX_WIDTH, PANEL_KEY_STEP } = RendererConstants;
    if (key === 'Home') return PANEL_MIN_WIDTH;
    if (key === 'End') return PANEL_MAX_WIDTH;
    const width = Dom.byId('dev-panel').getBoundingClientRect().width;
    return width + (key === 'ArrowLeft' ? PANEL_KEY_STEP : -PANEL_KEY_STEP);
  },

  /** Resizes the panel from the keyboard. */
  keydown(event) {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    oyaBrowser.resizeDevPanel(ShellLayout.keyWidth(event.key));
  },
};

oyaBrowser.onShellLayout(ShellLayout.apply);
ShellLayout.label(document.querySelector('.dev-panel-resize'));
document.querySelector('.dev-panel-resize').addEventListener('keydown', ShellLayout.keydown);
