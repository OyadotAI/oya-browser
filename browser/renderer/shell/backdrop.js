/**
 * The page backdrop: while a dialog detaches the page view, a still of the
 * page stands in for it so the window does not flash empty.
 */
/* global oyaBrowser, Dom */
/* exported Backdrop */

/** The page still. */
const Backdrop = {
  /** The latest request; a slower, older one never shows. */
  token: undefined,

  /** Shows (or hides) the still, telling the main process once it is painted. */
  async show(value) {
    const backdrop = Dom.byId('page-backdrop');
    Backdrop.token = value?.token;
    if (!value) return Backdrop.hide(backdrop);
    Backdrop.place(backdrop, value);
    await backdrop.decode().catch(() => {});
    if (Backdrop.token === value.token) await Backdrop.reveal(backdrop, value.token);
  },

  /** Shows the decoded still and, once it has been painted, tells the main process. */
  async reveal(backdrop, token) {
    backdrop.hidden = false;
    await new Promise((resolve) => requestAnimationFrame(resolve));
    oyaBrowser.backdropReady(token);
  },

  /** Positions the still over the page and loads it. */
  place(backdrop, { bounds, image }) {
    const px = (n) => n + 'px';
    Object.assign(backdrop.style, {
      left: px(bounds.x),
      top: px(bounds.y),
      width: px(bounds.width),
      height: px(bounds.height),
    });
    backdrop.src = image;
  },

  /** Hides the still. */
  hide(backdrop) {
    backdrop.hidden = true;
    backdrop.removeAttribute('src');
  },
};

oyaBrowser.onPageBackdrop(Backdrop.show);
