/**
 * /docs/<slug>.md: one part of the docs as Markdown, for agents. The parts
 * are listed in _docs/pages.ts; any other path under /docs is a 404.
 */
import { pageResponse } from '../_docs/markdown-route';

/** The route's parameters. */
interface Params {
  /** The path segment after /docs/, such as "sdk.md". */
  page: string;
}

/** What Next.js passes a dynamic route. */
interface Context {
  /** The parameters, resolved once awaited. */
  params: Promise<Params>;
}

/** One part of the docs, by its path segment. */
export async function GET(_request: Request, { params }: Context) {
  return pageResponse((await params).page);
}
