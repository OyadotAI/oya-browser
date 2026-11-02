# Reliability review

Discover the actual runtime and deployment model before applying a checklist. A desktop daemon,
serverless worker, CLI and Kubernetes service have different failure and recovery mechanisms.
Read startup, shutdown, persistence, queue ownership, and changed resource lifetimes.

Build a failure table: event, observable symptom, detection, owner, recovery, and maximum wait.
Inspect dependency outages, partial writes, disk full, restart, parent death, disconnect,
credential expiry, overload, and concurrent retries. Bound waits, queue sizes and buffers; check
retry backoff, jitter, idempotency, cancellation propagation, and terminal failure visibility.

For deployable services, verify drain/readiness semantics, measured resource limits and pool
budgets across replicas, compatible migrations, rollout ordering, rollback, and state isolation.
Apply platform-specific settings only when that platform is actually present. For local apps,
prioritize process cleanup, port ownership, resume behavior, and durable user state.

Check logs and health signals for actionable failure detection without credentials or sensitive
payloads. Separate measured timing/capacity from estimates. Propose a realistic failure
injection in scratch infrastructure; do not stop a user's running service to prove a point.

Use `../assets/findings.md`: what fails, reachable conditions, blast radius, evidence, smallest
repair, and verification. State coverage limits and recovery gaps. Keep implementation and live
infrastructure unchanged unless changes were assigned.
