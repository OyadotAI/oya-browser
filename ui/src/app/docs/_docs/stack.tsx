/**
 * The docs' "change one line" block: the line each existing stack (browser-use, Playwright, Browserbase, a coding
 * agent) changes to run on Oya, from the content the home page shares. It belongs to the intro, not to a docs
 * section, so it carries no section anchor: the quickstart stays the first section in the sidebar and in search.
 */
import { STACK_HEADING, STACK_LEAD, STACK_LINES } from '../../_home/stack-content';

/** The block, under the docs' opening paragraph. */
export function DocsStack() {
  return (
    <section aria-label={STACK_HEADING} className="mb-10">
      <h2 className="text-[20px] font-medium tracking-[-.02em] text-text mb-2">{STACK_HEADING}</h2>
      <p className="max-w-xl text-[14px] text-text-muted mb-4">{STACK_LEAD}</p>
      <ul className="grid gap-2">
        {STACK_LINES.map(({ id, who, lang, line, note }) => (
          <li key={id} className="rounded-xl border border-border bg-bg-card/40 px-4 py-3">
            <span className="text-[13.5px] font-medium text-text">{who}</span>
            <span className="ml-2 text-[12px] text-text-dim">{lang}</span>
            <pre className="mt-1.5 overflow-x-auto text-[12.5px] text-text">
              <code>{line}</code>
            </pre>
            <span className="mt-1 block text-[12px] text-text-dim">{note}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
