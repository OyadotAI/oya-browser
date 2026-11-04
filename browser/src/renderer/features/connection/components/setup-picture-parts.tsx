/** Small interface drawings form clear local illustrations without external image dependencies. */

/** An Oya window makes the browser side of every connection recognizable. */
export function BrowserPicture({
  done = false,
}: {
  /** Only real connection completion may draw the success mark. */ done?: boolean;
}) {
  return (
    <g className="picture-window">
      <rect width="184" height="128" rx="12" className="picture-paper" />
      <path d="M0 25H184" className="picture-rule" />
      <circle cx="14" cy="13" r="5" className="picture-green-fill" />
      <text x="25" y="17" className="picture-label">
        Oya Browser
      </text>
      <circle cx="157" cy="13" r="2" className="picture-dot" />
      <circle cx="165" cy="13" r="2" className="picture-dot" />
      <circle cx="173" cy="13" r="2" className="picture-dot" />
      <rect x="13" y="38" width="158" height="18" rx="5" className="picture-muted" />
      <text x="23" y="50" className="picture-small">
        github.com
      </text>
      <path d="M14 72H128M14 82H104" className="picture-copy" />
      <rect x="13" y="95" width="78" height="21" rx="6" className="picture-green-soft" />
      {done ? (
        <path d="m42 106 6 6 13-13" className="picture-green-line" />
      ) : (
        <text x="24" y="109" className="picture-small picture-accent-text">
          Your browser
        </text>
      )}
    </g>
  );
}

/** A terminal depicts the place local-agent users actually type their tasks. */
export function TerminalPicture({
  claude = false,
}: {
  /** Subscription tasks run in the unmodified coding agent. */ claude?: boolean;
}) {
  return (
    <g className="picture-window">
      <rect width="154" height="106" rx="11" className="picture-terminal" />
      <path d="M0 25H154" className="picture-terminal-rule" />
      <circle cx="13" cy="13" r="3" className="picture-terminal-dot" />
      <circle cx="24" cy="13" r="3" className="picture-terminal-dot" />
      <circle cx="35" cy="13" r="3" className="picture-terminal-dot" />
      <text x="13" y="47" className="picture-terminal-title">
        {claude ? 'Claude Code' : '>_ Your agent'}
      </text>
      <text x="13" y="67" className="picture-terminal-copy">
        › Find VS Code releases
      </text>
      <text x="13" y="83" className="picture-terminal-copy">
        › Version + release date
      </text>
      <path d="M13 94H22" className="picture-cursor" />
    </g>
  );
}

/** Chat bubbles show a task being handed to an assistant. */
export function ChatPicture() {
  return (
    <g>
      <path
        d="M9 0H125a9 9 0 0 1 9 9v26a9 9 0 0 1-9 9H24L9 56V44a9 9 0 0 1-9-9V9a9 9 0 0 1 9-9Z"
        className="picture-chat"
      />
      <text x="12" y="19" className="picture-label">
        Find VS Code releases
      </text>
      <text x="12" y="33" className="picture-small">
        version + release date
      </text>
      <rect x="23" y="64" width="111" height="28" rx="9" className="picture-paper" />
      <path d="M35 78H115" className="picture-copy" />
    </g>
  );
}

/** Approval is a schematic, never a second interactive sign-in surface. */
export function ApprovalPicture({
  provider = 'Oya workspace',
}: {
  /** Sign-in destination for this connection. */ provider?: string;
}) {
  return (
    <g className="picture-window">
      <rect width="145" height="124" rx="12" className="picture-paper" />
      <path d="M0 25H145" className="picture-rule" />
      <text x="13" y="17" className="picture-label">
        {provider}
      </text>
      <circle cx="73" cy="53" r="13" className="picture-green-soft" />
      <path d="M67 53h12M73 47v12" className="picture-green-line" />
      <text x="28" y="84" className="picture-label">
        Connect to Oya
      </text>
      <rect x="24" y="95" width="97" height="18" rx="5" className="picture-green-soft" />
      <text x="54" y="108" className="picture-small picture-accent-text">
        Approve
      </text>
    </g>
  );
}
