# aria_move maintenance roadmap

Updated 2026-10-31 after the project-wide initial audit. Application refactoring
has not started. The detailed plans are authoritative under docs:

- [Architecture and consolidation decisions](docs/ARCHITECTURE.md)
- [Prioritized tasks, dependencies and acceptance criteria](docs/REFACTORING_BACKLOG.md)
- [Measured validation baseline and limitations](docs/VALIDATION.md)

GitHub Actions and release-please automation have been removed as requested.
Local build, test, lint, security configuration and packaging tools are retained.
The existing user deletions in .github and .vscode are preserved.

Start with A01/A02 (trustworthy tests/checks) and B01 (unsafe recovery deletion),
then B02/B03 (dry-run and collision ownership), B04/B05 (verified tree copy and
resume identity), and B06 (cancellation/locks). Configuration consolidation C01
can proceed after A01. Follow the remaining dependencies in the backlog.

The initial audit replaces the previous limited roadmap's assumptions: the two
Windows failures need test corrections/investigation, not presumed application
fixes. Detailed historical claims are superseded by the dated measurements.
Future work must record before/after evidence and keep behavior fixes, structural
moves, formatting and policy changes separately reviewable. Tasks are plans,
not authorization to implement every item. Do not restore GitHub automation.
