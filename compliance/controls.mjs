/**
 * The HIPAA Security Rule controls this deployment claims, each with the code
 * that implements it and the check that proves it.
 *
 * A control with no check is a claim. Every entry here either names a check
 * that runs, or is marked a gap and says what would close it, an evidence pack
 * that hides its gaps is worth nothing to the auditor reading it.
 */

/** One control: what the rule asks, what implements it, and how it is proven. */
export const CONTROLS = [
  {
    id: '164.312(b)',
    title: 'Audit controls',
    requirement: 'Record and examine activity in systems that contain or use ePHI.',
    implements: ['server/src/platform/audit.ts', 'server/migrations/004_control_plane.sql'],
    check: 'audit.records',
  },
  {
    id: '164.312(c)(1)',
    title: 'Integrity',
    requirement: 'Protect ePHI from improper alteration or destruction.',
    implements: ['server/src/platform/audit-chain.ts', 'server/migrations/012_audit_tamper_evidence.sql'],
    check: 'audit.tamper-evidence',
  },
  {
    id: '164.312(c)(2)',
    title: 'Mechanism to authenticate ePHI',
    requirement: 'Corroborate that ePHI has not been altered or destroyed.',
    implements: ['server/src/platform/audit-chain.ts'],
    check: 'audit.append-only',
  },
  {
    id: '164.312(d)',
    title: 'Person or entity authentication',
    requirement: 'Verify that a person seeking access is the one claimed.',
    implements: ['server/src/modules/challenges/'],
    check: 'auth.mfa',
  },
  {
    id: '164.312(e)(1)',
    title: 'Transmission security',
    requirement: 'Guard against unauthorised access to ePHI in transit.',
    implements: ['server/src/modules/control/egress.ts', 'browser/governance.js'],
    check: 'egress.containment',
  },
  {
    id: '164.308(a)(4)',
    title: 'Information access management',
    requirement: 'Authorise access to ePHI only as appropriate, minimum necessary.',
    implements: ['server/src/modules/control/egress.ts'],
    check: 'egress.containment',
  },
  {
    id: '164.312(a)(2)(iv)',
    title: 'Encryption and decryption',
    requirement: 'Encrypt and decrypt ePHI, including credentials at rest.',
    implements: ['server/src/platform/secrets.ts', 'server/migrations/010_hash_api_keys.sql'],
    check: 'secrets.at-rest',
  },
  {
    id: '164.502(b)',
    title: 'Minimum necessary, recordings and logs',
    requirement: 'Limit PHI in what is stored to the minimum necessary.',
    implements: [],
    gap: 'Redaction covers secrets and form placeholders, not PHI. Screenshots and DOM snapshots of a payer portal are PHI at rest. Closing it needs a redaction pass over recordings and a retention clock per artifact.',
    check: 'phi.redaction',
  },
  {
    id: '164.308(b)(1)',
    title: 'Business associate contracts',
    requirement: 'A BAA with every subprocessor that handles ePHI.',
    implements: [],
    gap: 'Any third-party network on the ePHI path is a subprocessor needing a BAA. A residential proxy vendor cannot give one, so healthcare deployments must egress directly or through customer-owned infrastructure.',
    check: 'subprocessor.egress',
  },
  {
    id: '164.316(b)(2)',
    title: 'Documentation retention',
    requirement: 'Retain required documentation for six years.',
    implements: [],
    gap: 'No retention floor is enforced on the audit trail and no purge job exists for artifacts past their window. Needs a retention policy per artifact class with a six-year floor for audit records.',
    check: 'retention.policy',
  },
];
