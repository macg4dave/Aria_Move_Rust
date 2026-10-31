---
name: rust-check
description: Run the aria_move local quality gate and compare with measured evidence.
---

# rust-check

Follow AGENTS.md Validation and Definition of done, and PROJECT_RULES.md Errors,
output and checks. Run the documented fmt, clippy and test commands; on Unix also
check all features. Inspect effective lint flags and record a command-local strict
override with reliable restoration when necessary. Do not fix code unless asked.

Record revision, OS, toolchain, features, commands, exit codes, failed test names,
counts and diagnostics in docs/VALIDATION.md or the change report. Compare with a
fresh baseline; historical measurements are evidence, not a whitelist. An issue
absent from an old report is unclassified until investigated. Never infer that
formatting drift in a touched file was caused by this change without comparing it.
Report missing tools and native targets not exercised. Do not run real-storage
ZFS tests as part of the ordinary gate.
