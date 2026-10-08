/**
 * The conversation (`#chat-messages`): the empty state with its example
 * tasks, the messages, the cards of finished runs, and the live run last. It
 * keeps the newest entry in view. An agent reply is the escaped Markdown from
 * core/markdown.ts, set as HTML here and nowhere else.
 */
import { useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useViewModel } from '../../../hooks/index.ts';
import { Icon, Orb } from '../../../ui/index.ts';
import { ASK_TEXT, EXAMPLE_TASKS } from '../model/constants.ts';
import { isErrorReply, replyHtml } from '../model/reply.ts';
import type { AskViewModel, ChatMessage } from '../view-models/ask-view-model.ts';
import { FinishedRunCard, LiveRun } from './run-card.tsx';
import { SaveOffer } from './save-offer.tsx';

/** What the conversation's parts are given. */
interface AskProps {
  /** Opens a saved workflow in the library. */
  onViewPlaybook?: (name: string) => void;
  /** The conversation. */
  ask: AskViewModel;
}

/** What one message is given. */
interface MessageProps extends AskProps {
  /** The message. */
  message: ChatMessage;
}

/** The empty state: the orb, the question, and example tasks sent as if typed. */
function EmptyState({ ask }: AskProps) {
  return (
    <div className="chat-empty">
      <Orb size="lg" />
      <h2>
        What should I <em>do?</em>
      </h2>
      <p>Give Oya a task on this page or anywhere on the web. You can take the wheel at any moment.</p>
      <div className="chat-examples">
        {EXAMPLE_TASKS.map(({ task, label }) => (
          <button key={task} type="button" data-task={task} onClick={() => void ask.ask(task)}>
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}

/** The Copy button under a reply: copies its Markdown, and says so for a moment. */
function CopyButton({ ask, message }: MessageProps) {
  const done = useViewModel(ask).copied === message.id;
  const copy = () => void navigator.clipboard.writeText(message.content).then(() => ask.markCopied(message.id));
  return (
    <button className={done ? 'chat-copy done' : 'chat-copy'} onClick={(e) => (e.stopPropagation(), copy())}>
      <Icon name="copy" />
      <span>{done ? ASK_TEXT.copied : ASK_TEXT.copy}</span>
    </button>
  );
}

/**
 * An agent reply. Its HTML is the element's own content (the stylesheet styles
 * its blocks as direct children of `.chat-msg`), so the Copy button and the
 * offer are portalled in after it. A message never changes, so React never
 * resets that HTML under them.
 */
function AgentMessage({ ask, message, onViewPlaybook }: MessageProps) {
  const [box, setBox] = useState<HTMLDivElement | null>(null);
  const html = useMemo(() => ({ __html: replyHtml(message.content) }), [message.content]);
  const failed = isErrorReply(message.content);
  const extras = (
    <>
      {!failed && <CopyButton ask={ask} message={message} />}
      {message.offer && <SaveOffer offer={message.offer} onViewPlaybook={onViewPlaybook} />}
    </>
  );
  return (
    <>
      <div
        ref={setBox}
        className={failed ? 'chat-msg assistant error' : 'chat-msg assistant'}
        dangerouslySetInnerHTML={html}
      />
      {box && createPortal(extras, box)}
    </>
  );
}

/** Keeps the list scrolled to its newest entry whenever `deps` change. */
function useNewestInView(deps: unknown[]) {
  const list = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (list.current) list.current.scrollTop = list.current.scrollHeight;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  return list;
}

/** The conversation. */
export function Messages({ ask, onViewPlaybook }: AskProps) {
  const { items } = useViewModel(ask);
  const { card } = useViewModel(ask.run);
  const list = useNewestInView([items, card]);
  return (
    <div className="chat-messages" id="chat-messages" ref={list}>
      {!items.length && !card && <EmptyState ask={ask} />}
      {items.map((item) =>
        item.kind === 'run' ? (
          <FinishedRunCard key={item.id} item={item} onToggle={() => ask.toggleRun(item.id)} />
        ) : item.role === 'user' ? (
          <div key={item.id} className="chat-msg user">
            {item.content}
          </div>
        ) : (
          <AgentMessage key={item.id} ask={ask} message={item} onViewPlaybook={onViewPlaybook} />
        ),
      )}
      <LiveRun run={ask.run} />
    </div>
  );
}
