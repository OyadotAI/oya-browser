/**
 * The left column of the sign-in and sign-up pages: a kicker, a promise, the
 * reasons behind it and, optionally, the numbers that back them.
 */
import { Check } from 'lucide-react';

/** A pitch's content. */
export interface PitchContent {
  /** The small capitals above the heading. */
  kicker: string;
  /** The promise. */
  title: string;
  /** The paragraph under it. */
  lead: string;
  /** The reasons, title then body. */
  points: Array<[string, string]>;
  /** The numbers, figure then caption; none shows no row. */
  stats?: Array<[string, string]>;
}

/** What Oya's numbers are, shown under every pitch. */
export const STATS: Array<[string, string]> = [
  ['95%+', 'task success on measured portal runs'],
  ['0%', 'flagged as a bot on CreepJS'],
  ['0', 'model calls on a replayed run'],
];

/** Why a developer signs up; sign-in shows it too. */
export const DEVELOPER_PITCH: PitchContent = {
  kicker: 'OYA FOR DEVELOPERS',
  title: 'Give your agent a browser that gets in.',
  lead: 'A real browser with the automation inside it. Record a portal run once and it replays with no model in the loop.',
  points: [
    ['Free to start', 'The desktop app, 500 agent steps and an hour of cloud browsers a month. No card.'],
    ['Signs in like staff', 'A real browser with your own logins, so portals built to stop bots let your agent in.'],
    ['Replays with no model', 'A recorded run replays step for step, fast and with no model calls.'],
    ['No lock-in', 'Drive it from the SDK, MCP or Playwright, and export every playbook as Playwright code.'],
  ],
  stats: STATS,
};

/** Pitch's props. */
interface PitchProps {
  /** What to say. */
  content: PitchContent;
}

/** Stats' props. */
interface StatsProps {
  /** Figure then caption. */
  stats: Array<[string, string]>;
}

/** The numbers under the reasons. */
function Stats({ stats }: StatsProps) {
  return (
    <dl className="grid grid-cols-3 gap-3 border-t border-border pt-5">
      {stats.map(([figure, caption]) => (
        <div key={caption} className="flex flex-col-reverse justify-end gap-1">
          <dt className="text-xs text-text-muted">{caption}</dt>
          <dd className="font-display text-2xl font-bold text-accent">{figure}</dd>
        </div>
      ))}
    </dl>
  );
}

/** The column beside the card. */
export function Pitch({ content: { kicker, title, lead, points, stats } }: PitchProps) {
  return (
    <div className="flex-1 basis-[380px] space-y-5 pt-2">
      <p className="text-xs font-bold tracking-[0.15em] text-accent">{kicker}</p>
      <p className="font-display text-4xl font-bold leading-tight text-text">{title}</p>
      <p className="text-text-muted">{lead}</p>
      <ul className="space-y-3.5">
        {points.map(([point, body]) => (
          <li key={point} className="flex gap-3">
            <Check aria-hidden className="mt-0.5 h-5 w-5 flex-none text-accent" />
            <span>
              <span className="block text-sm font-semibold text-text">{point}</span>
              <span className="block text-sm text-text-muted">{body}</span>
            </span>
          </li>
        ))}
      </ul>
      {stats && <Stats stats={stats} />}
    </div>
  );
}
