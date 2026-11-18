/** Each narrative resolves to one existing product entry point rather than competing conversion paths. */
import { foundersCall } from '../_home/content';
import type { StoryAction } from './content';
/** Desktop readers can try the task in the existing application. */
export const DOWNLOAD_ACTION: StoryAction = {
  label: 'Download Oya Browser',
  href: '/#download',
  invitation: 'Start with the browser task you were about to do yourself.',
};
/** Engineering readers can bring a working automation into a project. */
export const BUILD_ACTION: StoryAction = {
  label: 'Start building',
  href: '/dashboard',
  invitation: 'Bring one working automation into your project, then try it with the next set of inputs.',
};
/** Regulated workflow buyers can scope a representative task with the founders. */
export const DEMO_ACTION: StoryAction = {
  label: 'Book a demo',
  href: foundersCall,
  invitation: 'Bring your SOP or screen recording. Scope your first workflow with the founders.',
};
/** Security review leads to the existing evidence and BAA request destination. */
export const BAA_ACTION: StoryAction = {
  label: 'Request the BAA',
  href: 'https://enterprise.oyabrowser.com/trust',
  invitation: 'Review the scope and dated evidence with your security and legal teams before you begin.',
};
