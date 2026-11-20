/**
 * Comparison table and side-by-side fit summary for public answer pages: keeps
 * semantic table markup crawlable while highlighting Oya, provenance dates and
 * exact "Not stated" cells without color-only signals.
 */
import { ArrowRight, CalendarCheck, Check } from 'lucide-react';
import trust from '../trust/trust.module.css';
import answers from './answers.module.css';
import { FIT_ICON_SIZE, NOT_STATED, OYA_COLUMN, PROVENANCE_ICON_SIZE } from './constants';
import type { AnswerFit, AnswerFitSide, AnswerSources, AnswerTable } from './content';

/** Provenance strip showing the vendor check date and a jump link to primary sources. */
function ProvenanceBar({ sources }: { /** Dated primary vendor sources for the page. */ sources: AnswerSources }) {
  return (
    <div className={answers.provenance}>
      <span className={answers.provenanceDate}>
        <CalendarCheck size={PROVENANCE_ICON_SIZE} aria-hidden="true" />
        <span>Sources checked {sources.checked}</span>
      </span>
      <a className={answers.provenanceLink} href="#sources">
        View primary sources ({sources.links.length})
      </a>
    </div>
  );
}

/** One comparison cell; exact "Not stated" renders as an inset dashed pill while qualified prose stays normal text. */
function TableCell({
  cell,
  isOya,
}: {
  /** Cell copy from the comparison table. */ cell: string;
  /** Whether this cell belongs to Oya's column. */ isOya: boolean;
}) {
  return (
    <td data-oya={isOya ? 'true' : undefined}>
      {cell === NOT_STATED ? <span className={answers.notStated}>{cell}</span> : cell}
    </td>
  );
}

/** Semantic comparison table with sticky criteria, Oya column emphasis and optional provenance strip. */
export function CompareTable({
  table,
  sources,
}: {
  /** Rows, columns and caption to render. */ table: AnswerTable;
  /** Optional dated sources rendered above the table scroll container. */ sources?: AnswerSources;
}) {
  const { caption, columns, rows } = table;
  const [, ...valueColumns] = columns;
  return (
    <div className={answers.compareWrap}>
      {sources && <ProvenanceBar sources={sources} />}
      <div
        className={answers.scroll}
        data-standalone={sources ? undefined : 'true'}
        role="region"
        aria-label={caption}
        tabIndex={0}
      >
        <table className={`${trust.table} ${answers.compare}`} data-columns={columns.length}>
          <caption>{caption}</caption>
          <thead>
            <tr>
              {columns.map((column) => (
                <th key={column} scope="col" data-oya={column === OYA_COLUMN ? 'true' : undefined}>
                  {column}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map(([name, ...cells]) => (
              <tr key={name}>
                <th scope="row">{name}</th>
                {cells.map((cell, i) => (
                  <TableCell key={valueColumns[i]} cell={cell} isOya={valueColumns[i] === OYA_COLUMN} />
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

/** One side of the decision fit card: heading and concise sourced criteria. */
function FitColumn({
  side,
  isOya,
}: {
  /** Heading and bullets for this option. */ side: AnswerFitSide;
  /** Whether this card summarizes Oya's fit. */ isOya: boolean;
}) {
  const Icon = isOya ? Check : ArrowRight;
  return (
    <div className={answers.fitCard} data-oya={isOya ? 'true' : undefined}>
      <h3>{side.heading}</h3>
      <ul>
        {side.points.map((point) => (
          <li key={point}>
            <Icon size={FIT_ICON_SIZE} aria-hidden="true" />
            <span>{point}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Side-by-side summary of when to choose Oya versus the compared tool or vendor. */
export function CompareFit({ fit }: { /** Sourced fit summary for both options. */ fit: AnswerFit }) {
  return (
    <div className={answers.fitGrid}>
      <FitColumn side={fit.oya} isOya />
      <FitColumn side={fit.other} isOya={false} />
    </div>
  );
}
