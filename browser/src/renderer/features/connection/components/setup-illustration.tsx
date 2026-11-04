/** Local interface illustrations explain setup with pictures rather than long paragraphs. */
import type { ReactNode } from 'react';
import { BrowserPicture, TerminalPicture, ChatPicture, ApprovalPicture } from './setup-picture-parts.tsx';
import { DashboardPicture } from './setup-dashboard-picture.tsx';
import './setup-illustration.css';

/** Scene names are explicit so copy and pictures cannot silently drift across setup paths. */
export type SetupScene =
  | 'tools'
  | 'assistant'
  | 'fleet'
  | 'chatgpt'
  | 'claude'
  | 'waiting'
  | 'connected'
  | 'key'
  | 'oya'
  | 'models'
  | 'import'
  | 'guide-browser'
  | 'guide-workspace'
  | 'guide-page'
  | 'guide-human';

/** Terminal and workspace-key routes both lead into this browser. */
function ToolsPicture() {
  return (
    <g>
      <path d="M105 56H124V39H167M121 165H202V151" className="picture-connector" />
      <g transform="translate(127 20)">
        <BrowserPicture />
      </g>
      <g transform="translate(6 54) scale(.85)">
        <TerminalPicture />
      </g>
      <rect x="23" y="151" width="108" height="29" rx="9" className="picture-paper" />
      <path d="M38 162a5 5 0 1 1 0 1m4 0h13m-5 0v5" className="picture-green-line" />
      <g className="picture-key-mask">
        <circle cx="66" cy="165" r="2" />
        <circle cx="73" cy="165" r="2" />
        <circle cx="80" cy="165" r="2" />
        <circle cx="87" cy="165" r="2" />
        <circle cx="98" cy="165" r="2" />
        <circle cx="105" cy="165" r="2" />
        <circle cx="112" cy="165" r="2" />
        <circle cx="119" cy="165" r="2" />
      </g>
      <circle cx="202" cy="151" r="4" className="picture-green-fill" />
    </g>
  );
}

/** The task bubble points toward the browser where the work will happen. */
function AssistantPicture() {
  return (
    <g>
      <path d="M112 93H165V61" className="picture-connector" />
      <g transform="translate(126 32)">
        <BrowserPicture />
      </g>
      <g transform="translate(5 47)">
        <ChatPicture />
      </g>
      <path d="m159 69 6-8 6 8" className="picture-green-line" />
    </g>
  );
}

/** The workspace view coordinates several browser windows without claiming a live connection. */
function FleetPicture() {
  return (
    <g>
      <path d="M80 115H213V52M169 116V163" className="picture-connector" />
      <g transform="translate(4 17) scale(.66)">
        <DashboardPicture area="browsers" />
      </g>
      <rect x="202" y="18" width="104" height="70" rx="9" className="picture-paper" />
      <rect x="188" y="35" width="104" height="70" rx="9" className="picture-muted" />
      <g transform="translate(137 70) scale(.94)">
        <BrowserPicture />
      </g>
    </g>
  );
}

/** Provider approval leads back to Oya; its line stays unfinished until verified completion. */
function ConnectionPicture({
  done = false,
  provider,
}: {
  /** Completion is supplied only by authenticated state. */ done?: boolean;
  /** Identifies the actual approval destination. */ provider?: string;
}) {
  return (
    <g>
      <path d="M128 98H186" className={done ? 'picture-connector' : 'picture-connector picture-pending'} />
      <g transform="translate(4 31)">
        <ApprovalPicture provider={provider} />
      </g>
      <g transform="translate(151 37) scale(.88)">
        <BrowserPicture done={done} />
      </g>
      {done && (
        <g>
          <circle cx="158" cy="96" r="17" className="picture-green-fill" />
          <path d="m150 96 6 6 11-12" className="picture-white-line" />
        </g>
      )}
    </g>
  );
}

/** Claude Code is visibly the place the person gives their task. */
function ClaudePicture() {
  return (
    <g>
      <path d="M130 99H178M161 92l8 7-8 7" className="picture-connector" />
      <g transform="translate(140 23) scale(.94)">
        <BrowserPicture />
      </g>
      <g transform="translate(4 67)">
        <TerminalPicture claude />
      </g>
    </g>
  );
}

/** Readable library data moves into Oya, distinct from protected signed-in sessions. */
function ImportPicture() {
  return (
    <g>
      <path d="M98 93H172" className="picture-connector" />
      <g transform="translate(139 29) scale(.94)">
        <BrowserPicture />
      </g>
      <rect x="7" y="33" width="108" height="45" rx="11" className="picture-paper" />
      <text x="24" y="61" className="picture-label">
        Chrome / Edge
      </text>
      <rect x="7" y="91" width="108" height="66" rx="11" className="picture-paper" />
      <path d="m34 104 4 9 10 1-8 7 3 10-9-5-9 5 3-10-8-7 10-1Z" className="picture-green-soft picture-green-stroke" />
      <circle cx="80" cy="120" r="15" className="picture-rule" />
      <path d="M80 110v11l7 5" className="picture-green-line" />
      <text x="23" y="146" className="picture-tiny">
        Bookmarks + history
      </text>
    </g>
  );
}

/** An account identity connects directly to the browser without another app confirmation. */
function OyaPicture() {
  return (
    <g>
      <path d="M80 100H190" className="picture-connector" />
      <g transform="translate(128 30)">
        <BrowserPicture />
      </g>
      <circle cx="65" cy="87" r="42" className="picture-paper" />
      <circle cx="65" cy="78" r="11" className="picture-green-fill" />
      <path d="M44 108c0-23 42-23 42 0" className="picture-green-line" />
      <text x="38" y="147" className="picture-label">
        Oya account
      </text>
    </g>
  );
}
/** Provider keys connect directly to Oya's model configuration, not to a separate terminal. */
function ModelsPicture() {
  return (
    <g>
      <path d="M67 42V147M67 95H173" className="picture-connector" />
      <g transform="translate(128 30)">
        <BrowserPicture />
      </g>
      <circle cx="65" cy="41" r="23" className="picture-paper" />
      <text x="59" y="46" className="picture-label">
        G
      </text>
      <circle cx="65" cy="95" r="23" className="picture-paper" />
      <text x="59" y="100" className="picture-label">
        C
      </text>
      <circle cx="65" cy="149" r="23" className="picture-paper" />
      <text x="55" y="154" className="picture-label">
        OR
      </text>
    </g>
  );
}
/** The guarded table keeps every setup view on a known local drawing. */
const SCENES: Record<SetupScene, () => ReactNode> = {
  tools: ToolsPicture,
  key: ToolsPicture,
  oya: OyaPicture,
  models: ModelsPicture,
  assistant: AssistantPicture,
  fleet: FleetPicture,
  chatgpt: () => <ConnectionPicture provider="ChatGPT" />,
  waiting: () => <ConnectionPicture />,
  connected: () => <ConnectionPicture done />,
  claude: ClaudePicture,
  import: ImportPicture,
  'guide-browser': () => <DashboardPicture area="browsers" />,
  'guide-workspace': () => <DashboardPicture area="workspace" />,
  'guide-page': () => <DashboardPicture area="page" />,
  'guide-human': () => <DashboardPicture area="human" />,
};

/** Decorative art reserves its aspect ratio; meaningful instructions remain selectable text outside it. */
export function SetupIllustration({
  scene,
}: {
  /** The concept illustrated by the surrounding caption. */ scene: SetupScene;
}) {
  return (
    <svg
      className="setup-illustration"
      viewBox="0 0 320 190"
      width="320"
      height="190"
      aria-hidden="true"
      focusable="false"
      data-scene={scene}
    >
      {Object.hasOwn(SCENES, scene) ? SCENES[scene]() : null}
    </svg>
  );
}
