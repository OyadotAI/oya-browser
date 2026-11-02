/**
 * The variables the steps use ({{name}} placeholders), each with its default and
 * whether it holds a secret. Changing one rewrites the definition, so the text
 * and this pane are always the same playbook.
 */
'use client';

import { EDITOR_TEXT } from './editor-text';
import { toText, variableRows, withDefault, withSecret } from './definition';
import type { Definition } from './editor-api';

/** Props for the pane. */
interface Props {
  /** The parsed definition. */
  definition: Definition;
  /** Replaces the editor text. */
  onChange: (text: string) => void;
}

/** One variable: its name, default and secret flag. */
function VariableRowView({
  d,
  name,
  value,
  secret,
  onChange,
}: Props & {
  /** Its name. */ d: Definition;
  /** The name. */ name: string;
  /** Its default. */ value: string;
  /** Secret. */ secret: boolean;
}) {
  return (
    <tr className="border-b border-border/60 last:border-0">
      <td className="px-3 py-2 font-mono text-text">{name}</td>
      <td className="px-3 py-2">
        <input
          className="field h-8 font-mono"
          type={secret ? 'password' : 'text'}
          aria-label={`${EDITOR_TEXT.defaultValue} for ${name}`}
          value={value}
          onChange={(e) => onChange(toText(withDefault(d, name, e.target.value)))}
        />
      </td>
      <td className="px-3 py-2 text-center">
        <input
          type="checkbox"
          className="accent-accent"
          aria-label={`${EDITOR_TEXT.secret}: ${name}`}
          checked={secret}
          onChange={(e) => onChange(toText(withSecret(d, name, e.target.checked)))}
        />
      </td>
    </tr>
  );
}

/** The variables as a table, or how to make one. */
export default function VariablesPane({ definition, onChange }: Props) {
  const rows = variableRows(definition);
  if (!rows.length) return <p className="py-6 text-[13px] text-text-muted">{EDITOR_TEXT.noVariables}</p>;
  return (
    <div className="space-y-2">
      <table
        className="w-full border-collapse overflow-hidden rounded-lg border border-border text-[13px]"
        aria-label="Variables"
      >
        <thead className="bg-bg-elevated text-left text-[11px] uppercase tracking-[0.1em] text-text-muted">
          <tr>
            <th className="px-3 py-2">{EDITOR_TEXT.variable}</th>
            <th className="px-3 py-2">{EDITOR_TEXT.defaultValue}</th>
            <th className="w-[80px] px-3 py-2 text-center">{EDITOR_TEXT.secret}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <VariableRowView key={r.name} d={definition} definition={definition} onChange={onChange} {...r} />
          ))}
        </tbody>
      </table>
      <p className="text-[12px] text-text-muted">{EDITOR_TEXT.secretHint}</p>
    </div>
  );
}
