---
name: writing-for-agents
description: "This skill should be used when creating or editing skills, agent definitions, AGENTS.md, CLAUDE.md, or reference documents intended for agents."
---

# Write instructions agents can follow

Define the target behavior, trigger, permitted actions, and completion evidence before adding
instructions. Preserve the user's scope and existing authorization. Keep instructions consistent
with the host's permissions and the assigned member's tools; a skill cannot grant new authority.

## Put information where it is needed

Keep the primary workflow in the main document. Co-locate each rule with its conditions and
caveats. Put specialized branches in supporting references and link them from the decision point
that needs them. State both what a reference contains and when to read it. Verify every local
link resolves within the shipped bundle.

Keep always-loaded descriptions short and specific. Give one trigger for each distinct branch
rather than a list of synonyms. Use the project's established vocabulary. Inline an essential
rule when a pointer would make it easy to miss; keep longer reference material behind a clear
pointer. Split documents only when the split helps a real reader or execution branch.

## Make completion observable

Write steps as concrete actions with evidence of completion: source locations inspected, inputs
covered, artifacts produced, or observations recorded. Define what to do when evidence is
unavailable. Separate observed results, inferences, and hypotheses. Avoid a requirement to claim
verification when the role cannot run the relevant check.

State the intended behavior directly. Keep hard prohibitions where they enforce real permission,
data, or tool constraints, and provide the permitted path. Resolve routine choices from the
assignment rather than inserting a new approval gate. Require user input only for material
unknown decisions or actions outside existing authorization.

## Package skills deliberately

Supply YAML `name` and `description`. Choose model invocation for a workflow the agent needs to
discover on its own. Set `disable-model-invocation: true` for an explicit command that should
run only when requested; use a short human-facing description and avoid preloading it into
unrelated members. Respect the host's supported metadata rather than assuming every platform
implements the same frontmatter.

Ship only resources referenced by a useful workflow. Preserve attribution and required licenses
for adapted material. Keep scripts and tools within the agent's actual permissions. Use Oya
Browser exclusively for browser instructions. Inspect available tool schemas instead of inventing
tool names or parameters.

## Prune and review

Keep each behavioral rule in one authoritative place. Reference discoverable configuration
instead of caching commands, ports, or directory lists that can go stale. Retain the non-obvious
reason and failure mode. Remove duplicated, irrelevant, or ambiguous instructions. Check likely
request examples against the trigger, workflow branches, and completion criteria; label this
as a static review. Runtime trials require the user's testing authorization.

Complete when an agent can identify when to load the document, what to do, what evidence to
return, and how to handle a blocker with its assigned tools.

Adapted from Matt Pocock's productivity skills; see [SOURCE.md](SOURCE.md) and [LICENSE](LICENSE).
