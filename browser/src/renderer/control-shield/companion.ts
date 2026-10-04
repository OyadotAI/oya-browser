/**
 * Oya's companion in the corner: its mood (which the orb's CSS animates), the
 * caption it speaks word by word, and the ring of light it pulses.
 */

/** The companion's mood: what it is doing, or '' at rest. */
export type Mood = 'scanning' | 'found' | 'acting' | '';

/** The companion's parts of the page. */
export interface CompanionParts {
  /** The #companion element, whose classes carry the mood. */
  root: HTMLElement;
  /** The caption's words (#bubble-text). */
  words: HTMLElement;
  /** The orb's pulse ring (.orb .pulse). */
  pulse: HTMLElement;
  /** The document, to make the words' spans. */
  document: Document;
}

/** The companion's face and what it says. */
export class Companion {
  /** The mood shown now. */
  mood: Mood = '';
  /** The elements it draws in. */
  private readonly parts: CompanionParts;

  /** `parts` is the companion's elements. */
  constructor(parts: CompanionParts) {
    this.parts = parts;
  }

  /** Sets the mood and the caption's words; no words hides it (the last words fade out with it). `busy` quickens the voice line. */
  say(mood: Mood, text: string, busy = false): void {
    this.mood = mood;
    const classes = ['companion', mood, text && 'talking', busy && 'busy'];
    this.parts.root.className = classes.filter(Boolean).join(' ');
    if (text) this.write(text);
  }

  /** Changes the words in place, without playing their arrival again: for a count ticking up. */
  count(text: string): void {
    this.parts.words.textContent = text;
  }

  /** Sends a ring of light out from the orb: the class comes off, a reflow forgets it, and it goes back on. */
  pulse(): void {
    const ring = this.parts.pulse;
    ring.classList.remove('go');
    void ring.offsetWidth;
    ring.classList.add('go');
  }

  /** Goes quiet, unless an action has taken the caption since: a show's end must not cut off what Oya is doing. */
  rest(): void {
    if (this.mood !== 'acting') this.say('', '');
  }

  /** The agent has been still: the caption goes, unless a read has taken it over. */
  quiet(): void {
    if (this.mood === 'acting') this.say('', '');
  }

  /** Writes the caption as one span per word, so each rises into place a moment after the one before. */
  private write(text: string): void {
    const parts = String(text).split(' ');
    const last = parts.length - 1;
    this.parts.words.replaceChildren(...parts.map((word, i) => this.word(i < last ? `${word} ` : word, i)));
  }

  /** One word of the caption, with its place in line for its delay. */
  private word(text: string, i: number): HTMLElement {
    const span = this.parts.document.createElement('span');
    Object.assign(span, { className: 'w', textContent: text });
    span.style.setProperty('--i', String(i));
    return span;
  }
}
