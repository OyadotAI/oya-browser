# Product scope

1. Reconstruct the user's current journey from the implementation and supplied evidence. Identify
   the person, task, entry point, current workaround, and moment of failure. Mark assumptions;
   never invent customer quotes, demand, adoption numbers, or measured time savings.
2. Translate the request into observable acceptance criteria. Cover first use, repeat use, and
   failure recovery. For an existing bug, use the reported behavior as the initial regression
   criterion; avoid turning the repair into an unrelated product strategy exercise.
3. Choose the smallest coherent slice. List dependencies and exclusions only when they affect
   implementation. Respect decisions the user already made. Recommend a cut with reasons;
   do not silently cancel authorized work or demand approval for ordinary implementation choices.
4. Rank competing work by user impact, frequency, cost, reversibility, and confidence in the
   evidence. Give a clear recommendation and what evidence would change it.
5. Use `../assets/brief.md` for the handoff. Define success in terms the designer, implementer,
   and QA can check. Distinguish acceptance criteria from metrics that need future collection.

Keep code unchanged. Research through Oya only when outside evidence affects the decision.
