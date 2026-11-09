/** Buyer details stay accessible on demand, keeping the main page concise. */
'use client';
import { useState } from 'react';
import { Plus, Minus } from 'lucide-react';
import { QUESTIONS } from './content';

/** Each question is an ordinary disclosure button, so Enter and Space work without custom keys. */
export function Faq() {
  const [open, setOpen] = useState<number | null>(null);
  return (
    <section className="section" id="faq" aria-labelledby="faq-h">
      <div className="wrap grid">
        <div className="span-5">
          <h2 id="faq-h">Check the details before you deploy.</h2>
          <a className="link" href="#contact">
            Talk to the founders
          </a>
        </div>
        <div className="faq span-6 start-7">
          {QUESTIONS.map(({ q, a }, index) => (
            <div key={q} className="faq__item">
              <h3>
                <button
                  id={`faq-question-${index}`}
                  type="button"
                  aria-expanded={open === index}
                  aria-controls={`faq-answer-${index}`}
                  onClick={() => setOpen(open === index ? null : index)}
                >
                  {q}
                  {open === index ? <Minus size={18} aria-hidden="true" /> : <Plus size={18} aria-hidden="true" />}
                </button>
              </h3>
              <div
                id={`faq-answer-${index}`}
                role="region"
                aria-labelledby={`faq-question-${index}`}
                hidden={open !== index}
              >
                <p>{a}</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
