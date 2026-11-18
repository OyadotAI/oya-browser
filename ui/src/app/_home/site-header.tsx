/**
 * The landing header.
 */
'use client';
import { Menu, X } from 'lucide-react';
import { OyaWordmark } from '@/components/oya-logo';
import { SiteLinks } from './site-header-links';
import styles from '../page.module.css';
import { useNavigation } from './use-navigation';

/** Wordmark and main navigation, with Download always in reach as the header sticks. */
export function SiteHeader() {
  const { state, close, toggle, escape } = useNavigation();
  return (
    <header className={styles.header} onKeyDown={escape}>
      <OyaWordmark />
      <button
        className={styles.menuToggle}
        aria-expanded={state.menu}
        aria-controls="main-links"
        onClick={() => toggle('menu')}
      >
        {state.menu ? <X size={18} /> : <Menu size={18} />} Menu
      </button>
      <SiteLinks open={state.menu} close={close} />
    </header>
  );
}
