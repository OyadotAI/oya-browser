/**
 * The docs sidebar, shared by the desktop aside and the mobile menu: search,
 * results, the section links and the footer links.
 */
'use client';

import type { PropsWithChildren, ReactNode, RefObject } from 'react';
import Link from 'next/link';
import {
  Search,
  X,
  ArrowLeft,
  ExternalLink,
  ChevronRight,
  BookOpen,
  Terminal,
  Code,
  Globe,
  Zap,
  Shield,
  Users,
  Layers,
} from 'lucide-react';
import { useDocsNav } from './nav';
import type { DocsSearch } from './use-docs-search';

/** A sidebar link: [section id, label], or the Swagger UI link. */
type NavEntry = [string, string] | 'swagger';

/** A group of sidebar links. */
interface NavGroup {
  /** The group's icon. */
  icon: typeof Layers;
  /** The group's heading. */
  label: string;
  /** Its links, in order. */
  links: NavEntry[];
}

/** Every sidebar group, in page order. */
const NAV_GROUPS: NavGroup[] = [
  {
    icon: Layers,
    label: 'Control Plane',
    links: [
      ['control-plane', 'Architecture'],
      ['comparison', 'Why Oya (10x Leap)'],
      ['routing-failover', 'Routing & Failover'],
      ['stealth-benchmarks', 'Stealth Benchmarks'],
    ],
  },
  {
    icon: Zap,
    label: 'Getting Started',
    links: [
      ['quickstart', 'Quickstart'],
      ['sdk', 'SDK'],
      ['cli', 'CLI'],
      ['create-key', 'Create API Key'],
      ['download', 'Desktop Sign-in'],
      ['connect', 'Connect'],
    ],
  },
  {
    icon: Users,
    label: 'Identity',
    links: [
      ['personas', 'Personas'],
      ['rotation', 'Rotation'],
      ['captcha', 'CAPTCHA'],
      ['mfa', 'MFA'],
    ],
  },
  {
    icon: BookOpen,
    label: 'AI Integration',
    links: [
      ['mcp-setup', 'MCP Setup'],
      ['cursor', 'Cursor'],
      ['claude-desktop', 'Claude Desktop'],
      ['claude-code', 'Claude Code'],
    ],
  },
  {
    icon: Terminal,
    label: 'MCP Tools',
    links: [
      ['analyze_page', 'analyze_page'],
      ['navigate', 'navigate'],
      ['click', 'click'],
      ['type', 'type'],
      ['press_key', 'press_key'],
      ['screenshot', 'screenshot'],
      ['scroll', 'scroll'],
      ['tabs', 'Tab management'],
      ['wait', 'wait'],
    ],
  },
  {
    icon: Shield,
    label: 'Anonymity',
    links: [
      ['anonymity', 'Overview'],
      ['fingerprint', 'Fingerprint Spoofing'],
      ['proxy-support', 'Proxy Support'],
      ['stealth', 'Anti-Detection'],
      ['choosing-a-persona', 'Choosing a persona'],
    ],
  },
  {
    icon: Globe,
    label: 'Dashboard',
    links: [
      ['dashboard-overview', 'Overview'],
      ['onboarding', 'Onboarding'],
      ['live-view', 'Live View'],
      ['settings', 'Settings'],
    ],
  },
  {
    icon: Code,
    label: 'API',
    links: [
      ['rest-api', 'REST API'],
      ['command-api', 'Command Reference'],
      'swagger',
      ['websocket', 'WebSocket Protocol'],
    ],
  },
];

/** A nav section's props. */
interface NavSectionProps extends PropsWithChildren {
  /** The group's icon. */
  icon: ReactNode;
  /** The group's heading. */
  label: string;
}

/** A labeled group of links. */
function NavSection({ icon, label, children }: NavSectionProps) {
  return (
    <div className="mb-5">
      <div className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-widest text-text-dim mb-2">
        {icon}
        {label}
      </div>
      <div className="flex flex-col gap-0.5">{children}</div>
    </div>
  );
}

/** A nav link's props. */
interface NavLinkProps extends PropsWithChildren {
  /** The section it goes to. */
  id: string;
}

/** A link to a section, highlighted while that section is in view. */
function NavLink({ id, children }: NavLinkProps) {
  const { active, navigate } = useDocsNav();
  return (
    <a
      href={'#' + id}
      aria-current={active === id ? 'location' : undefined}
      onClick={(e) => {
        e.preventDefault();
        navigate(id);
      }}
      className={`rounded-md px-2.5 py-1.5 text-[12.5px] transition-colors ${active === id ? 'bg-accent/8 text-accent font-medium' : 'text-text-muted hover:bg-text/5 hover:text-text'}`}
    >
      {children}
    </a>
  );
}

/** The link to the interactive API reference. */
function SwaggerLink() {
  return (
    <a
      href="/swagger"
      className="flex items-center gap-1 text-[13px] text-text-muted hover:text-text py-1 transition-colors"
    >
      Swagger UI <ExternalLink className="w-3 h-3" />
    </a>
  );
}

/** Every group of section links. */
function SectionLinks() {
  return NAV_GROUPS.map(({ icon: Icon, label, links }) => (
    <NavSection key={label} icon={<Icon className="w-3 h-3" />} label={label}>
      {links.map((link) =>
        link === 'swagger' ? (
          <SwaggerLink key="swagger" />
        ) : (
          <NavLink key={link[0]} id={link[0]}>
            {link[1]}
          </NavLink>
        ),
      )}
    </NavSection>
  ));
}

/** Props for the parts that use the search. */
interface SearchProps {
  /** The search state. */
  search: DocsSearch;
}

/** The search box's props. */
interface SearchBoxProps extends SearchProps {
  /** The input, which Escape blurs. */
  inputRef: RefObject<HTMLInputElement | null>;
}

/** The search input, with a clear button once something is typed. */
function SearchBox({ search, inputRef }: SearchBoxProps) {
  return (
    <div className="relative mb-5">
      <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-dim pointer-events-none" />
      <input
        ref={inputRef}
        type="text"
        placeholder="Search docs…"
        aria-label="Search documentation"
        className="w-full bg-bg-sunken border border-border rounded-lg py-2.5 pl-8 pr-8 text-text text-sm outline-none placeholder:text-text-dim focus:border-indigo transition-colors"
        value={search.query}
        onChange={(e) => search.onInput(e.target.value)}
        onKeyDown={search.onKeyDown}
      />
      {search.query && (
        <button
          aria-label="Clear search"
          onClick={search.clear}
          className="absolute right-2 top-1/2 -translate-y-1/2 text-text-dim hover:text-text transition-colors"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      )}
    </div>
  );
}

/** The results dropdown, while there is a query. */
function SearchResults({ search }: SearchProps) {
  if (!search.results) return null;
  return (
    <div className="mb-4 bg-bg-card border border-border rounded-xl overflow-hidden shadow-lg shadow-black/30">
      {search.results.length === 0 ? (
        <div className="text-xs text-text-dim px-3 py-2">No results</div>
      ) : (
        search.results.map((hit, i) => (
          <button
            key={i}
            onClick={() => search.pick(hit.id)}
            className="w-full text-left block text-xs px-3 py-2 text-text-muted hover:bg-bg-elevated hover:text-text transition-colors cursor-pointer border-b border-border last:border-b-0"
          >
            <span className="text-text-dim text-[11px] leading-snug">{hit.snippet}</span>
          </button>
        ))
      )}
    </div>
  );
}

/** Home, console, llms.txt and the search hint. */
function SidebarFooter() {
  return (
    <div className="mt-auto pt-5 border-t border-border space-y-1.5">
      <Link href="/" className="flex items-center gap-1.5 text-xs text-text-dim hover:text-text transition-colors">
        <ArrowLeft className="w-3 h-3" /> Back to home
      </Link>
      <Link
        href="/dashboard"
        className="flex items-center gap-1.5 text-xs text-text-dim hover:text-text transition-colors"
      >
        <ChevronRight className="w-3 h-3" /> Open Dashboard
      </Link>
      <a href="/llms.txt" className="block text-[11px] text-text-dim/60 hover:text-text-dim transition-colors mt-2">
        llms.txt (plain text for AI)
      </a>
      <div className="text-[10px] text-text-dim/40 mt-2">
        Press <kbd className="bg-bg-elevated border border-border rounded px-1 py-0.5 font-mono text-[10px]">/</kbd> to
        search
      </div>
    </div>
  );
}

/** The sidebar's contents. */
export function Sidebar({ search, inputRef }: SearchBoxProps) {
  return (
    <>
      <div className="mb-6 flex items-center justify-between">
        <span className="eyebrow text-text-dim">Documentation</span>
        <span className="rounded border border-border px-1.5 py-0.5 font-mono text-[10px] text-text-dim">v1</span>
      </div>
      <SearchBox search={search} inputRef={inputRef} />
      <SearchResults search={search} />
      <SectionLinks />
      <SidebarFooter />
    </>
  );
}
