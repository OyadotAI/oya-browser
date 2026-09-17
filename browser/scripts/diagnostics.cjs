const PRIVATE = /password|passwd|secret|token|authorization|cookie|api.?key|otp|one.?time|fingerprint|origins|text|value|body|payload|params|headers/i;
function safeUrl(raw) {
  try { const url = new URL(raw); url.username = ''; url.password = ''; url.search = ''; url.hash = ''; return url.toString(); } catch { return '[redacted URL]'; }
}
function redact(input, secrets = [], depth = 0) {
  if (depth > 8) return '[omitted]';
  if (typeof input === 'string') {
    let text = input.replace(/https?:\/\/[^\s"<>]+/g, safeUrl).replace(/Bearer\s+[^\s"']+/gi, 'Bearer [redacted]');
    for (const value of secrets) if (value && String(value).length > 2) text = text.split(String(value)).join('[redacted]');
    return text.slice(0, 4000);
  }
  if (Array.isArray(input)) return input.slice(0, 100).map(value => redact(value, secrets, depth + 1));
  if (input && typeof input === 'object') return Object.fromEntries(Object.entries(input).slice(0, 100).map(([key, value]) => [key, PRIVATE.test(key) ? '[redacted]' : redact(value, secrets, depth + 1)]));
  return input;
}
module.exports = { redact, safeUrl };
