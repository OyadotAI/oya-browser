/**
 * RB2B's visitor identification: loads their script once, the way their own
 * snippet does. The layout renders it only when the operator set RB2B_ID, and
 * the component only on public pages (lib/public-pages.ts).
 */

/** Where RB2B serves its script from. */
const SCRIPT_HOST = 'https://ddwl4m2hdecbv.cloudfront.net';
/** What an RB2B account id looks like; anything else is not put into a script URL. */
const ACCOUNT_ID = /^[A-Z0-9]{6,32}$/;

/** RB2B's marker on window, which their snippet uses to load only once. */
type Rb2bWindow = Window & {
  /** Set once RB2B's script has been added. */
  reb2b?: {
    /** Always true once set. */
    loaded: boolean;
  };
};

/** Whether `id` is a well-formed RB2B account id. */
export const validRb2bId = (id: string | undefined): id is string => !!id && ACCOUNT_ID.test(id);

/** Adds RB2B's script to the page, once per page load. */
export function loadRb2b(id: string) {
  const w = window as Rb2bWindow;
  if (w.reb2b || !validRb2bId(id)) return;
  w.reb2b = { loaded: true };
  const script = document.createElement('script');
  script.async = true;
  script.src = `${SCRIPT_HOST}/b/${id}/${id}.js.gz`;
  document.head.appendChild(script);
}
