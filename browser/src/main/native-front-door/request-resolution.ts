/** Only commands that resolve an existing intercepted request may bypass waiting navigation work. */
export function resolvesRequest(method: string): boolean {
  return method === 'Fetch.continueRequest' || method === 'Fetch.failRequest' || method === 'Fetch.fulfillRequest';
}
