# Design signals for investigation

Use these Fowler-style heuristics to ask where complexity causes a concrete maintenance cost.
Apply repository conventions first. Treat these as leads, not automatic findings:

- **Unclear names:** trace whether a caller misunderstands a contract before proposing a rename.
- **Duplication:** compare behavior and reasons for change before extracting similar syntax.
- **Feature envy or message chains:** check whether a caller knows details another owner should hide.
- **Data clumps or primitive obsession:** look for invalid combinations or repeated validation that a domain type could contain.
- **Repeated dispatch:** inspect whether multiple sites disagree about the same variants.
- **Shotgun surgery:** identify one logical change that repeatedly requires scattered edits.
- **Divergent change:** identify unrelated responsibilities coupled in one place.
- **Speculative generality:** verify whether extra parameters, hooks, and adapters serve an actual requirement.
- **Middle man:** check whether forwarding adds policy, compatibility, or ownership before removing it.
- **Refused bequest:** inspect whether inheritance requires invalid or ignored behavior; consider composition where it clarifies the contract.

Record a finding only with evidence appropriate to the requested review scope. Avoid declaring
a defect from a line count or a preferred design pattern.
