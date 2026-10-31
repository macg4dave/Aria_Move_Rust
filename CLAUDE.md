# CLAUDE.md

Shared Rust guidance and project contracts:

@AGENTS.md
@PROJECT_RULES.md

Maintenance plans and historical observations live in docs/REFACTORING_BACKLOG.md.

## Claude Code specifics

- Detect the host OS; on Windows do not use --all-features (xattr is Unix-only).
- Plan symbol moves before multi-file refactors.
- Editor diagnostics provide fast feedback; the documented gate verifies Rust changes.
- Local /rust-check and /rust-refactor skills reference the shared rules and
  docs/VALIDATION.md; measure current results instead of assuming an old baseline.
