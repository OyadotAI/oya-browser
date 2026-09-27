/**
 * What a model call cost the operator, from its token counts: the hosted
 * model is the operator's key, so its use is billed to the person at cost
 * plus the markup their plan's Stripe price carries.
 */
import { MODEL_PRICES, UNKNOWN_MODEL_PRICE } from './constants.ts';

/** Token counts as the model API reports them. */
type Used = {
  /** Tokens read. */
  prompt_tokens?: number;
  /** Tokens written. */
  completion_tokens?: number;
};

/** A model name without its router prefix. */
const bare = (model: string) => String(model || '').replace(/^openai\//, '');

/** Whether the hosted model is one this server knows the price of: the only ones it runs on its own key. */
export const priced = (model: string) => Object.hasOwn(MODEL_PRICES, bare(model));

/** The cost of one call in micro-USD, rounded up; a model the table does not know is priced at the dearest. */
export function llmCost(model: string, used: Used | undefined) {
  const price = priced(model) ? MODEL_PRICES[bare(model)] : UNKNOWN_MODEL_PRICE;
  return Math.ceil((used?.prompt_tokens || 0) * price.input + (used?.completion_tokens || 0) * price.output);
}
