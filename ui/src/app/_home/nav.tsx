/** The landing navigation, with React-owned product and small-screen disclosures. */
'use client';
import { useRef } from 'react';
import Link from 'next/link';
import Image from 'next/image';
import { usePathname } from 'next/navigation';
import { ChevronDown, Menu, X } from 'lucide-react';
import { SCREENSHOT_HEIGHT, SCREENSHOT_WIDTH } from './constants';
import { OyaLogo } from '@/components/oya-logo';
import { foundersCall } from './content';
import { Arrow } from './arrow';
import { PRODUCT, WORDS } from './nav-content';
import { useNavigation } from './use-navigation';
import styles from './nav.module.css';

/** All original Product destinations, reachable by pointer and keyboard. */
function ProductPanel({ open }: { /** Whether its trigger is expanded. */ open: boolean }) {
  return (
    <div className={styles.productPanel} id="product-links" hidden={!open}>
      {PRODUCT.map(({ group, links }) => (
        <div key={group} className={styles.column}>
          <p>{group}</p>
          {links.map(([item, what, href]) => (
            <a key={item} href={href}>
              {item} <span>{what}</span>
            </a>
          ))}
        </div>
      ))}
      <Link className={styles.menuCard} href="/#playbooks">
        <Image
          src="/playbooks-list.jpg"
          alt="The playbooks list in the Oya console"
          width={SCREENSHOT_WIDTH}
          height={SCREENSHOT_HEIGHT}
          loading="lazy"
        />
        <span>Playbooks</span>
        <span>Every run, saved as code</span>
      </Link>
    </div>
  );
}

/** Disclosure state shared by the header's small presentation components. */
type Navigation = ReturnType<typeof useNavigation>;

/** The whole Product row is one disclosure button: a split link and chevron left most of the row dead on phones. */
function ProductDisclosure({ state, toggle }: Navigation) {
  return (
    <div className={styles.product}>
      <button
        className={styles.expand}
        type="button"
        aria-expanded={state.product}
        aria-controls="product-links"
        onClick={() => toggle('product')}
      >
        Product <ChevronDown size={14} aria-hidden="true" />
      </button>
      <ProductPanel open={state.product} />
    </div>
  );
}

/** Following any navigation destination dismisses the small-screen disclosure. */
function NavLinks(props: Navigation) {
  const pathname = usePathname();
  return (
    <nav
      className={styles.links}
      data-open={props.state.menu}
      aria-label="Main"
      id="main-links"
      onClick={(event) => {
        if ((event.target as Element).closest('a')) props.close();
      }}
    >
      <ProductDisclosure {...props} />
      {WORDS.map(({ word, href }) => (
        <a key={word} href={href} aria-current={pathname === href ? 'page' : undefined}>
          {word}
        </a>
      ))}
    </nav>
  );
}

/** The main action stays visible beside the small-screen menu trigger. */
function NavActions({ state, toggle }: Navigation) {
  return (
    <>
      <a
        className={`btn btn--primary ${styles.demo}`}
        href={foundersCall}
        data-track="cta_clicked"
        data-track-label="header_founders"
      >
        Book a demo
        <Arrow />
      </a>
      <button
        className={styles.toggle}
        type="button"
        aria-expanded={state.menu}
        aria-label="Menu"
        aria-controls="main-links"
        onClick={() => toggle('menu')}
      >
        {state.menu ? <X size={18} aria-hidden="true" /> : <Menu size={18} aria-hidden="true" />}
      </button>
    </>
  );
}

/** A compact header keeps its disclosures local to the landing page. */
export function Nav() {
  const headerRef = useRef<HTMLElement>(null);
  const nav = useNavigation(headerRef);
  return (
    <header
      ref={headerRef}
      className={styles.header}
      onKeyDown={nav.escape}
      onBlur={(event) => {
        // Safari never focuses a tapped button or link, so a tap blurs with no relatedTarget; closing then ate every tap in the open menu.
        const next = event.relatedTarget;
        if (next && !event.currentTarget.contains(next)) nav.close();
      }}
    >
      <div className={`wrap ${styles.bar}`}>
        <a className={styles.skip} href="#top">
          Skip to content
        </a>
        <Link className="nav__logo" href="/">
          <OyaLogo size={24} /> Oya
        </Link>
        <NavLinks {...nav} />
        <NavActions {...nav} />
      </div>
    </header>
  );
}
