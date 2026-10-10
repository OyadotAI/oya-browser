/** Fence asynchronous session setup against identity and governance transitions. */
interface PolicyPersona {
  /** Current immutable fingerprint input. */
  active: unknown;
  /** Changes even when a transition returns to the original identity. */
  policyEpoch?: symbol;
}
/** Governance is included because proxy installation and script policy must agree. */
interface PolicyGovernance {
  /** Current server-authorized policy. */
  configuration: unknown;
}
/** Capture both object ownership and semantic values before any asynchronous native mutation. */
export function capturePersonaPolicy(persona: PolicyPersona, governance: PolicyGovernance | null) {
  const profile = persona.active,
    epoch = persona.policyEpoch;
  const key = personaPolicyKey(persona, governance);
  return () => {
    const changed = persona.active !== profile || persona.policyEpoch !== epoch;
    if (changed || personaPolicyKey(persona, governance) !== key)
      throw Error('Native persona or governance changed during session setup');
  };
}

/** Serialize mutable policy inputs for comparison after native asynchronous operations. */
function personaPolicyKey(persona: PolicyPersona, governance: PolicyGovernance | null): string {
  return JSON.stringify([persona.active, governance?.configuration]);
}
