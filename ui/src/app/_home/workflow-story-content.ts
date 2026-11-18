/** Qualified workflow messaging is shared by the landing page and relevant payer-portal guides. */
export const MAINTENANCE_BODY =
  'A stopped replay can be repaired by an agent. Server replay can save repaired steps; desktop alternative-locator repairs remain paused drafts for review.';
/** Reader-facing repair answers distinguish an attempt from success and explain the desktop review boundary. */
export const REPAIR_FAQ =
  'A changed page can stop a replay. An agent can try to repair it. Server repairs can be saved; on desktop, review suggested changes before use. Desktop alternative-locator repairs are paused drafts, and assertions are not auto-repaired.';
/** Reusing recorded steps and attempting a repair are different model-use paths. */
export const REPLAY_MODEL_FAQ =
  'Recorded steps replay without a model. An agent may be called to try a repair when those steps no longer fit the page. Server repairs can be saved; desktop repair suggestions need review before use.';
/** Portal documents complement electronic claim-status transactions; availability is payer dependent. */
export const PORTAL_CONTEXT = {
  heading: 'Go beyond claim status.',
  body: 'Complement 276/277 claim-status feeds with portal access to available denial letters, prior-authorization communications and appeal documents. Availability varies by payer and plan.',
};
/** Public primary sources substantiate the document categories without implying Oya coverage. */
export const PORTAL_SOURCES = {
  checked: '2026-11-18',
  links: [
    {
      label: 'CMS: electronic claim-status requests and responses',
      url: 'https://www.cms.gov/medicare/coding-billing/electronic-billing/claim-status-request-response',
    },
    {
      label: 'UnitedHealthcare: portal document library and availability',
      url: 'https://www.uhcprovider.com/en/resource-library/provider-portal-resources/document-library.html',
    },
    {
      label: 'UnitedHealthcare: EDI 275 claim attachments',
      url: 'https://www.uhcprovider.com/en/resource-library/news/2025/clearinghouse-option-edi-275-claim-attachment.html',
    },
  ],
};
