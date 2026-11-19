/** Wraps the landing header or footer in the landing's tokens, so every public page shares one of each. */
import type { ReactNode } from 'react';
import landing from './landing.module.css';
import bold from './bold.module.css';
import styles from './site-chrome.module.css';

/** Applies the landing's colours, type and helper classes without adding a layout box, so the header still sticks. */
export function SiteChrome({ children }: { /** The landing header or footer. */ children: ReactNode }) {
  return <div className={`${landing.page} ${bold.bold} ${styles.chrome}`}>{children}</div>;
}
