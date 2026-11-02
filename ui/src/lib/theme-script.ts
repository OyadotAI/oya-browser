/**
 * The theme script app/layout.tsx injects into <head> before any bundle, so
 * the first paint already has the right theme and nothing flashes.
 *
 * It is a string, not a function: it has to run before hydration, and a
 * module would arrive too late. The rule it holds is the whole theme model of
 * the console. A saved choice in localStorage('oya_theme') wins; with none,
 * the page follows the OS, at load and whenever the OS switches, and every
 * change fires the `oya-theme` event the toggles listen on, so their icons
 * follow. The saved value is read again at every OS change, so a click made
 * after load still beats a later OS flip. Reading localStorage has its own
 * try: a profile with storage blocked throws on the access itself, and the
 * listener must still register so that profile follows the OS. Only 'light'
 * and 'dark' count as saved; anything else in storage is treated as nothing.
 */

/** The script source. Rendered verbatim into the nonced <script> in app/layout.tsx. */
export const THEME_SCRIPT =
  "try{var d=document.documentElement,m=matchMedia('(prefers-color-scheme: dark)')," +
  "g=function(){try{return localStorage.getItem('oya_theme')}catch{return null}}," +
  "a=function(){var t=g();d.dataset.theme=t==='light'||t==='dark'?t:m.matches?'dark':'light'};" +
  "a();m.addEventListener('change',function(){a();dispatchEvent(new Event('oya-theme'))})}catch{}";
