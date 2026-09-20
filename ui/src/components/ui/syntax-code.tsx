/**
 * Syntax-highlighted code, rendered as React text and spans (never HTML)
 * with Prism's tokenizer.
 */
'use client';

import { useMemo, type ReactNode } from 'react';
import Prism from 'prismjs';
import 'prismjs/components/prism-typescript';
import 'prismjs/components/prism-json';
import 'prismjs/components/prism-bash';

Prism.manual = true;
/** The languages the console highlights. */
type Language = 'typescript' | 'json' | 'bash';

/** Guesses the language: JSON by its opening bracket, shell by its first command. */
function detect(code: string): Language {
  if (/^\s*[\[{]/.test(code)) return 'json';
  if (/^\s*(?:\$ |#|npm |npx |oya |git |curl |export |xattr |open |docker |cd |\.\/)/.test(code)) return 'bash';
  return 'typescript';
}

/** Render tokens as React text and spans, so code never becomes executable HTML. */
function render(tokens: string | Prism.Token | (string | Prism.Token)[]): ReactNode {
  if (typeof tokens === 'string') return tokens;
  if (Array.isArray(tokens))
    return tokens.map((token, index) =>
      typeof token === 'string' ? (
        token
      ) : (
        <span key={index} className={`token ${token.type}`}>
          {render(token.content)}
        </span>
      ),
    );
  return <span className={`token ${tokens.type}`}>{render(tokens.content)}</span>;
}

/** SyntaxCode's props. */
interface Props {
  /** The source to highlight. */
  code: string;
  /** Forces a language instead of detecting it. */
  language?: Language;
}

/** Highlighted `code`. */
export default function SyntaxCode({ code, language }: Props) {
  const lang = language || detect(code);
  const tokens = useMemo(() => Prism.tokenize(code, Prism.languages[lang]), [code, lang]);
  return (
    <code className={`syntax-code language-${lang}`} data-language={lang}>
      {render(tokens)}
    </code>
  );
}
