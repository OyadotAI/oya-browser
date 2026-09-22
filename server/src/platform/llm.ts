/**
 * Facade over llm/: one model request in the shared OpenAI chat-completions shape,
 * served by Claude, Gemini or any OpenAI-compatible endpoint. Kept at this path so
 * its callers and exported names stay stable.
 */
export { chatCompletion, toGemini, fromGemini, providerFor, LlmError } from './llm/index.ts';
export type { ChatRequest, ChatResponse, LlmProvider } from './llm/index.ts';
