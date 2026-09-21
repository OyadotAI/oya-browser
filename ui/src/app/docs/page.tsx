/**
 * The docs page: a fixed header, a sidebar with search and section links
 * (a dialog on mobile), and the sections themselves. Every heading id is an
 * anchor that links, the sidebar and the search jump to, so they must not
 * change. The sections live in _sections, the page's machinery in _docs.
 */
'use client';

import { useCallback, useState } from 'react';
import Dialog from '@/components/ui/dialog';
import { DocsHeader } from './_docs/docs-header';
import { DocsIntro } from './_docs/intro';
import { DocsNav, useDocsNavState, useSlashToSearch } from './_docs/nav';
import { Sidebar } from './_docs/sidebar';
import { useDocsSearch } from './_docs/use-docs-search';
import { ControlPlaneDocs } from './_sections/control-plane';
import { GettingStartedDocs } from './_sections/getting-started';
import { AiIntegrationDocs } from './_sections/ai-integration';
import { McpToolsDocs } from './_sections/mcp-tools';
import { IdentityDocs } from './_sections/identity';
import { AnonymityDocs } from './_sections/anonymity';
import { DashboardDocs } from './_sections/dashboard';
import { ApiDocs } from './_sections/api';

/** The mobile menu's state, the navigation and the search. */
function useDocsPage() {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const closeMenu = useCallback(() => setMobileMenuOpen(false), []);
  const nav = useDocsNavState(closeMenu);
  const [search, inputRef] = useDocsSearch(closeMenu);
  useSlashToSearch();
  return { mobileMenuOpen, setMobileMenuOpen, closeMenu, nav, search, inputRef };
}

/** The documentation. */
export default function DocsPage() {
  const { mobileMenuOpen, setMobileMenuOpen, closeMenu, nav, search, inputRef } = useDocsPage();
  const sidebar = <Sidebar search={search} inputRef={inputRef} />;
  return (
    <DocsNav.Provider value={nav}>
      <div className="docs-page min-h-screen bg-bg">
        <DocsHeader onOpenMenu={() => setMobileMenuOpen(true)} />
        {mobileMenuOpen && (
          <Dialog open onClose={closeMenu} title="Documentation" size="sm">
            <nav aria-label="Documentation sections">{sidebar}</nav>
          </Dialog>
        )}
        <aside
          aria-label="Documentation sections"
          className="hidden lg:block fixed bottom-0 left-0 top-[72px] z-30 w-[260px] border-r border-border bg-bg-card/25 flex-col px-6 py-7 overflow-y-auto"
        >
          {sidebar}
        </aside>
        <main className="min-w-0 lg:ml-[260px] px-5 pt-28 pb-24 sm:px-10 lg:px-14 lg:pt-32">
          <div className="max-w-[800px] mx-auto">
            <DocsIntro />
            <ControlPlaneDocs />
            <GettingStartedDocs />
            <AiIntegrationDocs />
            <McpToolsDocs />
            <IdentityDocs />
            <AnonymityDocs />
            <DashboardDocs />
            <ApiDocs />
          </div>
        </main>
      </div>
    </DocsNav.Provider>
  );
}
