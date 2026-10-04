/**
 * The Ask pane (`#pane-chat`): the toolbar (profile picker, Model, Clear), the
 * Sign in and model cards, the conversation, and the input bar.
 */
import { useViewModel } from '../../../hooks/index.ts';
import { Button, IconButton } from '../../../ui/index.ts';
import './ask.css';
import type { RendererServices } from '../../../app/services.ts';
import type { AskViewModel } from '../view-models/ask-view-model.ts';
import { personaLocked } from '../view-models/persona-view-model.ts';
import { ModelCard } from './model-card.tsx';
import { Messages } from './messages.tsx';
import { Composer } from './composer.tsx';

/** What the Ask pane is given. */
export interface AskPaneProps {
  /** The conversation and its collaborators. */
  ask: AskViewModel;
  /** The workspace panel: the pane in view, and whether it is open. */
  panel: RendererServices['panel'];
}

/** What the toolbar and the Sign in card are given. */
interface AskPartProps {
  /** The conversation and its collaborators. */
  ask: AskViewModel;
}

/** The pane's toolbar: the profile picker, the Model button and Clear. */
function AskToolbar({ ask }: AskPartProps) {
  const { sending } = useViewModel(ask);
  const persona = useViewModel(ask.persona);
  const { signedIn } = useViewModel(ask.model);
  return (
    <div className="studio-header chat-toolbar">
      <select
        id="chat-persona"
        aria-label="Profile"
        title="The profile Oya browses as. Switching reopens your tabs with that profile's cookies."
        disabled={personaLocked(persona.ready, sending)}
        value={persona.active}
        onChange={(e) => ask.persona.change(e.target.value)}
      >
        {persona.options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <Button
        id="chat-model-open"
        title="Change the AI model key Ask runs on"
        hidden={!signedIn}
        onClick={() => void ask.model.openCard()}
      >
        Model
      </Button>
      <IconButton id="chat-clear" label="Clear chat" data-icon="trash" onClick={() => ask.clear()} icon="trash" />
    </div>
  );
}

/** "Sign in to use Ask", while this browser has no project. */
function SignInCard({ ask }: AskPartProps) {
  const { signedIn } = useViewModel(ask.model);
  return (
    <div className="chat-card" id="chat-signin" hidden={signedIn}>
      <h2>Sign in to use Ask</h2>
      <p>Ask runs on your Oya project. Signing in takes one click in your web browser.</p>
      <Button type="button" variant="primary" id="chat-signin-button" onClick={() => ask.model.signIn()}>
        Sign in with Oya
      </Button>
    </div>
  );
}

/** The Ask pane, shown while the panel is on Ask. */
export function AskPane({ ask, panel }: AskPaneProps) {
  const { pane } = useViewModel(panel);
  return (
    <div className={pane === 'chat' ? 'dev-pane active' : 'dev-pane'} id="pane-chat">
      <AskToolbar ask={ask} />
      <SignInCard ask={ask} />
      <ModelCard model={ask.model} />
      <Messages ask={ask} />
      <Composer ask={ask} panel={panel} />
    </div>
  );
}
