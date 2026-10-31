---
name: rust-refactor
description: Perform an authorized behavior-preserving aria_move refactor using shared rules.
---

# rust-refactor

Follow AGENTS.md Refactoring playbook and Scope and approval, plus PROJECT_RULES.md
One implementation per responsibility. Read the relevant task in
docs/REFACTORING_BACKLOG.md; task listings alone do not authorize implementation.

Measure baseline using the shared gate. Inventory callers, module declarations,
public exports and cfg branches; record test discovery names/counts. Compare
contracts before combining code. Plan each symbol move as old::path -> new::path.
Execute the authorized recipe, remove obsolete private implementations and preserve
public compatibility paths. Routine multi-file work already requested needs no
extra approval; ask only for decisions outside the authorized scope under the
shared rules. Keep bug fixes and regression tests separate from structural moves.

Repeat the same checks and compare evidence; explain removed or renamed tests,
symbol moves and outstanding defects. Keep task status and dated measurements
in the backlog/validation documents rather than copying them into this skill.
