# Refactoring backlog

The requested backlog has been implemented. No implementation tasks are deferred.
Current ownership is described in [ARCHITECTURE.md](ARCHITECTURE.md); executed
checks and platform coverage are in [VALIDATION.md](VALIDATION.md).

| Completed IDs | Result |
|---|---|
| A01–A03 | Corrected test assumptions, strict formatting/lints, deterministic private tests and isolated subprocess fixtures. |
| B01–B02 | Removed heuristic recovery deletion; dry runs perform no unintended writes. |
| B03–B05 | Exclusive publication, verified directory staging and fresh staging instead of trusting legacy partial files. |
| B06 | Cancellable locks/copies/walks, explicit lock policy and typed interruption. |
| C01–C02 | One config load/map/override path, exclusive templates and shared path/platform policy. |
| C03–C05 | Shared disk queries, source/naming rules and metadata handling. |
| D01–D02 | Retained diagnostic causes, shared output policy and removal of dormant alternatives. |
| D03 | Tracked Cargo lock, fixed parser advisories, current security configuration and working local Docker/ZIP/Debian packaging. |

The public replacement behavior change was explicitly approved: existing
move/copy destinations now produce an error instead of being overwritten.
Existing user deletions of ROADMAP.md and README.md.backup were preserved.
No commits, pushes, publication or GitHub automation were introduced.
