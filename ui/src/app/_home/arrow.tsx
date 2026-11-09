/**
 * The kit's drawn arrow for the primary button: its stem and head move 3px on hover through
 * the motion layer. One arrow style a page, only on the actions that need one.
 */

/** The arrow glyph, as the kit draws it. */
export function Arrow() {
  return (
    <svg className="m-arrow" viewBox="0 0 12 12" aria-hidden="true">
      <path className="m-arrow__stem" d="M1 6h8" />
      <path className="m-arrow__head" d="M4.5 2.5 8 6l-3.5 3.5" />
    </svg>
  );
}
