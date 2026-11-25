/** Annotated console scenes follow its real tabs and selected-browser panel without claiming live state. */

/** Each guide scene emphasizes one existing control-plane surface. */
export type DashboardArea = 'browsers' | 'workspace' | 'page' | 'human';

/** The header locates the project switcher and Settings above the console tabs. */
function DashboardChrome({ area }: { /** The one highlighted header control. */ area: DashboardArea }) {
  return (
    <g>
      <rect x="8" y="8" width="304" height="174" rx="13" className="picture-paper" />
      <path d="M8 43H312M8 72H312" className="picture-rule" />
      <circle cx="25" cy="26" r="7" className="picture-green-fill" />
      <text x="38" y="30" className="picture-label">
        Oya
      </text>
      <rect
        x="119"
        y="15"
        width="121"
        height="23"
        rx="6"
        className={area === 'workspace' ? 'picture-highlight' : 'picture-muted'}
      />
      <text x="129" y="30" className="picture-label">
        My workspace⌄
      </text>
      <circle cx="262" cy="26" r="7" className="picture-rule" />
      <circle cx="262" cy="26" r="2" className="picture-dot" />
      <text x="248" y="40" className="picture-tiny">
        Settings
      </text>
      <circle cx="293" cy="26" r="8" className="picture-muted" />
    </g>
  );
}

/** These are the current console's navigation labels rather than invented task controls. */
function DashboardTabs() {
  return (
    <g>
      <rect x="17" y="48" width="60" height="20" rx="5" className="picture-green-soft" />
      <text x="24" y="61" className="picture-small picture-accent-text">
        Browsers
      </text>
      <text x="91" y="61" className="picture-small">
        Profiles
      </text>
      <text x="139" y="61" className="picture-small">
        Playbooks
      </text>
      <text x="202" y="61" className="picture-small">
        Routines
      </text>
      <text x="262" y="61" className="picture-small">
        Control
      </text>
    </g>
  );
}

/** A fleet-table row makes the current desktop easy to recognize. */
function BrowserRows({
  area,
}: {
  /** Browser selection is highlighted on the first guide step. */ area: DashboardArea;
}) {
  return (
    <g>
      <text x="21" y="91" className="picture-label">
        Browsers
      </text>
      <rect
        x="20"
        y="103"
        width="280"
        height="31"
        rx="6"
        className={area === 'browsers' ? 'picture-highlight' : 'picture-muted'}
      />
      <rect x="29" y="111" width="22" height="15" rx="3" className="picture-paper" />
      <path d="M29 115H51" className="picture-rule" />
      <text x="62" y="123" className="picture-label">
        This desktop
      </text>
      <text x="254" y="123" className="picture-small">
        Desktop
      </text>
      <path d="M29 151H119M165 151H224M258 151H290" className="picture-copy" />
    </g>
  );
}

/** The selected browser's control sits above its URL, preview and activity. */
function BrowserDetail({
  area,
}: {
  /** Page inspection and control each highlight the actual panel. */ area: DashboardArea;
}) {
  return (
    <g>
      <text x="21" y="91" className="picture-label">
        Browsers
      </text>
      <rect x="20" y="103" width="116" height="31" rx="6" className="picture-green-soft" />
      <text x="30" y="123" className="picture-label">
        This desktop
      </text>
      <rect x="148" y="78" width="154" height="96" rx="8" className="picture-paper" />
      <text x="157" y="91" className="picture-label">
        This desktop
      </text>
      <text x="157" y="108" className="picture-small">
        Agent control
      </text>
      <rect
        x="232"
        y="96"
        width="62"
        height="18"
        rx="5"
        className={area === 'human' ? 'picture-highlight' : 'picture-muted'}
      />
      <text x="237" y="108" className="picture-small picture-accent-text">
        Take control
      </text>
      <BrowserPage area={area} />
    </g>
  );
}

/** The URL and live page are viewable before the person takes control. */
function BrowserPage({
  area,
}: {
  /** Highlights the read-only page preview on its guide step. */ area: DashboardArea;
}) {
  return (
    <g>
      <rect x="157" y="120" width="136" height="14" rx="4" className="picture-muted" />
      <text x="166" y="130" className="picture-tiny">
        github.com
      </text>
      <rect
        x="157"
        y="139"
        width="136"
        height="19"
        rx="4"
        className={area === 'page' ? 'picture-highlight' : 'picture-muted'}
      />
      <path d="M166 149H232" className="picture-copy" />
      <text x="158" y="168" className="picture-tiny">
        Activity
      </text>
    </g>
  );
}

/** The project picker shows a workspace selection; API access remains in real Settings. */
function WorkspaceMenu() {
  return (
    <g>
      <rect x="119" y="44" width="121" height="61" rx="7" className="picture-paper" />
      <text x="129" y="60" className="picture-label">
        Projects
      </text>
      <rect x="125" y="68" width="109" height="22" rx="5" className="picture-highlight" />
      <text x="133" y="82" className="picture-small">
        My workspace
      </text>
    </g>
  );
}

/** A consistent schematic highlights real console locations rather than live account data. */
export function DashboardPicture({ area }: { /** One conceptual stop in the walkthrough. */ area: DashboardArea }) {
  return (
    <g>
      <DashboardChrome area={area} />
      <DashboardTabs />
      {area === 'page' || area === 'human' ? <BrowserDetail area={area} /> : <BrowserRows area={area} />}
      {area === 'workspace' && <WorkspaceMenu />}
    </g>
  );
}
