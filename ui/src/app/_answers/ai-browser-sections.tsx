/** Desktop chapters explain starting, delegation, intervention, reuse and team scope in a connected sequence. */
import { Bot, Cloud, FileCode2, Hand, Laptop, MessageSquare, Plug, Repeat2, SlidersHorizontal } from 'lucide-react';
import { Reveal } from '@/components/ui/reveal';
import type { AnswerComposition, AnswerSection } from './content';
import styles from './answers.module.css';

/** The start choices are real entry paths rather than a second list of unrelated features. */
function StartChoices({ section }: Props) {
  const icons = [MessageSquare, Plug],
    single = section.items?.length === 1;
  return (
    <>
      <p>{section.body}</p>
      <div className={styles.choices} data-single={single}>
        {section.items?.map((item, index) => {
          const Icon = single ? Plug : (icons[index] ?? Plug);
          return (
            <Reveal key={item} order={index}>
              <Icon size={28} aria-hidden="true" />
              <p>{item}</p>
            </Reveal>
          );
        })}
      </div>
    </>
  );
}
/** The person and agent have distinct tabs throughout the delegated task. */
function SeparateTabs({ section }: Props) {
  return (
    <>
      <p>{section.body}</p>
      <div className={styles.lanes}>
        <Reveal>
          <Laptop size={30} aria-hidden="true" />
          <strong>Your tabs</strong>
          <span>Keep your work open.</span>
        </Reveal>
        <Reveal order={1}>
          <Bot size={30} aria-hidden="true" />
          <strong>The agent’s tab</strong>
          <span>Follow the task here.</span>
        </Reveal>
      </div>
    </>
  );
}
/** The illustration describes ownership without pretending its label is a live product control. */
function Takeover({ section }: Props) {
  return (
    <>
      <p>{section.body}</p>
      <figure className={styles.storyScene}>
        <Hand size={38} aria-hidden="true" />
        <figcaption>You decide when work continues.</figcaption>
      </figure>
    </>
  );
}
/** The file visual appears only after the chapter has defined a playbook in ordinary language. */
function SavedSteps({ section }: Props) {
  const steps = [
    ['Saved steps', FileCode2],
    ['New inputs', SlidersHorizontal],
    ['Run again', Repeat2],
  ] as const;
  return (
    <>
      <p>{section.body}</p>
      <div className={styles.savedSteps}>
        {steps.map(([label, Icon], index) => (
          <Reveal key={label} order={index}>
            <Icon size={28} aria-hidden="true" />
            <span>{label}</span>
          </Reveal>
        ))}
      </div>
    </>
  );
}
/** Team deployment is the consequence of growing work, with HIPAA scope still visible in its paragraph. */
function DeploymentLanes({ section }: Props) {
  return (
    <>
      <p>{section.body}</p>
      <div className={styles.lanes}>
        <Reveal>
          <Laptop size={30} aria-hidden="true" />
          <span>Desktop</span>
          <strong>One person</strong>
        </Reveal>
        <Reveal order={1}>
          <Cloud size={30} aria-hidden="true" />
          <span>Cloud &amp; enterprise</span>
          <strong>Teams</strong>
        </Reveal>
      </div>
    </>
  );
}
/** Stable data keys keep copy edits from silently changing a chapter's visual meaning. */
interface Props {
  /** The reader's current narrative chapter. */ section: AnswerSection;
}
/** All approved desktop scenes are selected by explicit composition metadata. */
const SCENES: Record<AnswerComposition, (props: Props) => React.ReactNode> = {
  start: StartChoices,
  tabs: SeparateTabs,
  takeover: Takeover,
  saved: SavedSteps,
  deployment: DeploymentLanes,
};

/** Unconfigured content remains complete prose; no heading text acts as an implicit router. */
export function AiBrowserSection({ section }: Props) {
  const key = section.composition,
    Scene = key && Object.hasOwn(SCENES, key) ? SCENES[key] : null;
  return Scene ? (
    <Scene section={section} />
  ) : (
    <>
      <p>{section.body}</p>
      <ul>
        {section.items?.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </>
  );
}
