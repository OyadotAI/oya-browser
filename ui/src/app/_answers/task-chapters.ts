/**
 * The task-specific chapters of the four payer-portal pages: the portal steps, what the agent reads
 * and writes, and where a person takes over. Each page has its own, because the shared workflow
 * chapters say the same thing on every page and a searcher asking about one task was reading four
 * pages that differed by a paragraph. Steps are the usual shape of the task, never a payer's exact
 * form; where they differ by payer, the copy says so.
 */
import type { AnswerSection } from './content';

/** Prior authorization writes to the payer, so its chapters dwell on scope and the final submission. */
export const PRIOR_AUTH_CHAPTERS: AnswerSection[] = [
  {
    heading: 'Map the prior-authorization steps on one portal.',
    body: 'The steps below are the usual shape of the task. The fields, their order and the records a payer asks for differ by payer and by service, so map one payer before an agent attempts it.',
    items: [
      'Sign in to the payer portal as a staff member, with the stored password and TOTP seed sealed from the model.',
      'Find the member and open a new prior-authorization request.',
      'Fill in the request form with the details of the case.',
      'Submit the form, if your process lets the agent submit.',
      'Read what the portal shows after submission, so the request can be tracked later.',
    ],
    layout: 'flow',
  },
  {
    heading: 'Decide what the agent reads and what it writes.',
    body: 'Filing a prior authorization writes to the payer’s system, so it deserves a narrower scope than a lookup. List both sides before the first run.',
    items: [
      'Reads: the request form, the portal’s response after submission, and the status of requests already filed.',
      'Writes: the values typed into the form, and the submission itself.',
      'Inputs: the details that change per case become variables of the saved playbook.',
      'Record: each step, by the agent or a person, is a row on the audit trail.',
    ],
  },
  {
    heading: 'Keep a person at the points that need judgment.',
    body: 'An agent can fill in and submit the form, but your team decides where it stops. A person can watch the session live, take over and hand it back.',
    items: [
      'A sign-in that needs a person, such as a text-message code.',
      'A question on the form that the case details do not answer.',
      'Records a staff member has to choose for this request.',
      'The final submission, where your process wants a person to send it.',
    ],
    layout: 'controls',
  },
];

/** An eligibility check is a lookup, so its chapters keep it read-only and route unclear results. */
export const ELIGIBILITY_CHAPTERS: AnswerSection[] = [
  {
    heading: 'Trace one coverage check from sign-in to result.',
    body: 'An eligibility check is a lookup: nothing is filed, and the answer is read from the page. The search fields and the layout of the result differ by payer, so trace one portal first.',
    items: [
      'Sign in as a staff member; the stored credentials stay sealed from the model.',
      'Open the portal’s eligibility search.',
      'Enter the member details your staff uses for the search.',
      'Read the coverage, copay and deductible the portal shows for that member.',
      'Repeat with the next patient’s details as inputs.',
    ],
    layout: 'flow',
  },
  {
    heading: 'Keep the check read-only.',
    body: 'An eligibility playbook types search fields and reads a result. It has no reason to change anything on the portal, and its scope can say so.',
    items: [
      'Reads: coverage, copay and deductible, as that payer’s result page presents them.',
      'Writes on the portal: the member details typed into the search form, and nothing else.',
      'Hosts: the project’s browsers reach only the hosts you allow.',
      'Record: each step, including the read, is a row on the audit trail.',
    ],
  },
  {
    heading: 'Send the unclear results to staff.',
    body: 'A check that returns no clear answer is more useful flagged than guessed. Decide which results stop the run and go to a person, who can open the same session and take over.',
    items: [
      'The portal finds no member, or more than one, for the details entered.',
      'The result page does not show the copay or deductible your staff expects.',
      'The portal asks for a text-message code or signs the session out.',
      'The page has changed and the replay stops.',
    ],
    layout: 'controls',
  },
];

/** Claim status is one lookup repeated on a schedule, so its chapters cover the list and the hand-off. */
export const CLAIM_STATUS_CHAPTERS: AnswerSection[] = [
  {
    heading: 'Lay out one claim lookup, then the list behind it.',
    body: 'Claim status work is one lookup repeated across a list of claims, on a schedule. How a portal searches for a claim, and how much remittance detail it shows, differs by payer.',
    items: [
      'Sign in to the payer portal as a staff member, with credentials sealed from the model.',
      'Search for the claim with the claim details your billing staff uses.',
      'Read where the claim stands.',
      'Open the remittance detail, where the portal shows it, and read it.',
      'Move to the next claim on the list, on the schedule you set.',
    ],
    layout: 'flow',
  },
  {
    heading: 'List what the lookup reads, and confirm it writes nothing.',
    body: 'A claim status playbook reads. The only things it types are search fields, so a run should leave the claim exactly as it found it.',
    items: [
      'Reads: the claim’s status and its remittance detail on that payer’s portal.',
      'Reads, where available: documents attached to the claim. Availability varies by payer and plan.',
      'Writes: the claim details typed into the portal’s search, and nothing else.',
      'Record: every scheduled read is a row on the audit trail.',
    ],
  },
  {
    heading: 'Route the claims that need a decision to billing staff.',
    body: 'A status is information; what to do about it is a billing decision. The playbook reads and reports, and a person picks up the claims that need work.',
    items: [
      'A claim the portal cannot find with the details supplied.',
      'A denied claim: the next step is denial and appeal work, which is its own workflow.',
      'A session that needs sign-in again partway through the list.',
      'A portal change that stops the replay; an agent can try a repair.',
    ],
    layout: 'controls',
  },
];

/** The umbrella page sorts every portal task by the same steps and by what it reads and writes. */
export const PAYER_PORTAL_CHAPTERS: AnswerSection[] = [
  {
    heading: 'Break any portal task into the same steps.',
    body: 'Prior authorizations, eligibility checks, claim status, denials and appeals look different, but each one is a sign-in, a search, a form or a result page, and a record of what happened. The details differ by payer and by task.',
    items: [
      'Sign in as a staff member; stored passwords and TOTP seeds are sealed and never shown to the model.',
      'Find the patient, member or claim the task is about.',
      'For a lookup, read the result page. For a submission, fill in the payer’s form.',
      'Submit, if the task files something.',
      'Read what the portal returns, and supply the changing details as inputs next time.',
    ],
    layout: 'flow',
  },
  {
    heading: 'Group tasks by what they read and what they write.',
    body: 'Lookups and submissions carry different risk. Start with a lookup if you want the first workflow to change nothing on the payer’s side.',
    items: [
      'Eligibility checks read coverage, copay and deductible. They write only search fields.',
      'Claim status reads where a claim stands and its remittance detail. It writes only search fields.',
      'Prior authorizations write a request to the payer and read its status afterwards.',
      'Denials and appeals read the denial reason and write an appeal on the payer’s own form.',
    ],
    layout: 'choices',
  },
  {
    heading: 'Decide where staff step in, task by task.',
    body: 'Every session can be watched live, taken over and handed back, so the question is where you want a person by default. Set that per task, not once for the whole portal.',
    items: [
      'Sign-ins that need a person, such as a text-message code.',
      'Submissions your process wants reviewed before they are sent.',
      'Results the saved steps do not expect: no match, a changed page, a signed-out session.',
      'Anything that calls for clinical or billing judgment.',
    ],
    layout: 'controls',
  },
];
