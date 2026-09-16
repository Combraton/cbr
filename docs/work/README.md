# Durable task work

Use this directory for plans and handoffs that span sessions. GitHub issues own task assignment and progress; these files preserve evidence and decisions too detailed for a short issue.

Current state: [STATE](STATE.md). Parent tracking issue: [#1](https://github.com/Combraton/cbr/issues/1).

- **Readiness**, settled by [ADR 001](../decisions/001-standalone-v0.1-scope-and-stack.md): [discussion](readiness/TALK.md), [protocol pin and contract mapping](readiness/PROTOCOL-PIN.md), [release scope and milestones](readiness/RELEASE-SCOPE.md) and [stack evidence](readiness/STACK.md).
- **M1, complete:** the walking skeleton over the real command path, [issue #3](https://github.com/Combraton/cbr/issues/3). The closing record is [m1/CLOSEOUT.md](m1/CLOSEOUT.md).
- **M2, complete:** Knowledge, [issue #12](https://github.com/Combraton/cbr/issues/12).
- **M3, active:** Context, retrieval and the deterministic packet compiler, [issue #15](https://github.com/Combraton/cbr/issues/15), in four pull requests: m3a the Context profile, m3b retrieval and the dependency evaluator, m3c the compiler on a real repository (J1, J8), m3d the journey-6 pilot.

Use the shared [task](https://github.com/Combraton/combraton/blob/main/docs/templates/TASK.md), [handoff](https://github.com/Combraton/combraton/blob/main/docs/templates/HANDOFF.md) and [integration evidence](https://github.com/Combraton/combraton/blob/main/docs/templates/INTEGRATION.md) formats. Link the issue and record the current head. Do not copy full transcripts, credentials or another task's private context. On restart, compare the handoff with current Git and process state before acting.
