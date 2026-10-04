/**
 * The shell page's entry: builds the ViewModels over the page's bridge and
 * clock, renders the shell, and puts the ViewModels on window.oyaShell for the
 * Electron tests, which drive the page through them. The tokens and the
 * page-wide rules load first; every other stylesheet comes in with its view.
 */
import { createRoot } from 'react-dom/client';
import '../core/tokens.css';
import './shell.css';
import { ShellRoot } from './shell-root.tsx';
import { ShellViewModels } from './view-models.ts';

declare global {
  /** What the shell page exposes. */
  interface Window {
    /** Every ViewModel, for the Electron tests. */
    oyaShell: ShellViewModels;
  }
}

const vms = new ShellViewModels({
  bridge: window.oyaBrowser,
  frames: { request: (callback) => requestAnimationFrame(callback), cancel: (handle) => cancelAnimationFrame(handle) },
  clipboard: navigator.clipboard,
  platform: navigator.platform,
});
window.oyaShell = vms;
const mount = document.getElementById('root');
if (!mount) throw new Error('The shell page has no #root');
createRoot(mount).render(<ShellRoot vms={vms} />);
