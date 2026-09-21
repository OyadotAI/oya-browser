/**
 * Shapes the project switcher works with: projects, key metadata, the
 * switcher's panel state and the context its actions share.
 */
/** A project the account can open, as GET /auth/projects lists it. */
export interface Project {
  /** Project id (`prj_…`). */
  id: string;
  /** Display name. */
  name: string;
  /** The account's role there, shown for shared projects. */
  role: string;
  /** Whether the account owns it; owners get the options menu. */
  owner?: boolean;
}

/** Key metadata; plaintext is fetched separately by an owner action. */
export interface OwnedKey {
  /** Key id. */
  id: string;
  /** The key's visible prefix. */
  prefix?: string;
  /** The project the key opens. */
  project?: string;
  /** The owner's label for it. */
  label?: string;
}

/** A failed request, with the server's status and error code when it gave them. */
export interface Failure extends Error {
  /** The HTTP status. */
  status?: number;
  /** The server's machine-readable error code. */
  code?: string;
}

/** The panels the switcher can show besides the project list. */
export type Form = 'new' | 'join' | 'import' | 'rename' | 'delete' | 'manage' | 'key';

/** The panels that are a form with a submit button. */
export type SubmitForm = Exclude<Form, 'manage' | 'key'>;

/** Everything the switcher's popover shows. */
export interface PickerState {
  /** Whether the popover is open. */
  open: boolean;
  /** The open panel; null is the project list. */
  form: Form | null;
  /** The project a panel acts on. */
  target: Project | null;
  /** A key shown once to be copied; never persisted. */
  revealedKey: string;
  /** The name field. */
  name: string;
  /** The invitation code or API key field. */
  secret: string;
  /** The project search. */
  query: string;
  /** The last failure, shown in an alert. */
  error: string;
  /** An owned project whose sealed key this server cannot open; its original API key repairs it. */
  restore: Project | null;
  /** Whether the key was just copied. */
  copied: boolean;
  /** The project being switched to. */
  pending: string | null;
  /** Whether a request is in flight. */
  busy: boolean;
}

/** Shows a message in the dashboard's toast. */
export type Toast = (msg: string, type?: 'success' | 'error' | 'info') => void;

/** A mutable number that survives renders. */
export interface Counter {
  /** The latest switch's number. */
  current: number;
}

/** What the project session needs to open, renew and forget projects. */
export interface Session {
  /** The account token; null for key-only sign-in, which has no projects. */
  token: string | null;
  /** Hands the console its new credential. */
  setApiKey: (credential: string, project: string | null) => void;
  /** Reports outcomes. */
  toast: Toast;
  /** Every switch takes a number; a slower, older request never overrides a newer one. */
  opening: Counter;
  /** Marks which project is open. */
  setCurrentId: (id: string | null) => void;
  /** Stores the latest project and key listing. */
  setListing: (listing: Listing) => void;
}

/** The account's projects and the keys it owns. */
export interface Listing {
  /** Projects the account can open. */
  projects: Project[];
  /** Keys the account owns, for the prefix in the options panel. */
  keys: OwnedKey[];
}

/** What every switcher action and panel receives. */
export interface PickerContext extends Listing {
  /** The popover's state as of this render. */
  ui: PickerState;
  /** Merges changes into the popover's state. */
  patch: (changes: Partial<PickerState>) => void;
  /** The session behind the switcher. */
  session: Session;
  /** The open project. */
  currentId: string | null;
  /** Gives focus back to the switcher button. */
  focusTrigger: () => void;
}

/** Props of a panel that only needs the switcher. */
export interface PanelProps {
  /** The switcher's context. */
  c: PickerContext;
}

/** Props of a panel that acts on one project. */
export interface TargetProps extends PanelProps {
  /** The project it acts on. */
  target: Project;
}

/** Props of a form panel. */
export interface FormProps extends PanelProps {
  /** Which form is open. */
  form: SubmitForm;
}

/** The server's answer carrying a project's API key. */
export interface KeyAnswer {
  /** The API key. */
  key: string;
}

/** The server's answer carrying a one-hour project credential. */
export interface AccessAnswer {
  /** The credential. */
  token: string;
}

/** The server's answer naming the project just joined. */
export interface JoinAnswer {
  /** The project id. */
  project: string;
}
