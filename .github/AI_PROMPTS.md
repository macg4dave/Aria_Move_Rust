# AI Prompts Guide for Aria_Move_Rust

This file contains a short, practical set of guardrails, templates and a recommended workflow to use when asking an AI to edit this repository. Put this file in `.github/AI_PROMPTS.md` and copy the relevant template into the assistant prompt when requesting changes.

---

## Repo at-a-glance (copy into prompts)
- Language: Rust (edition 2024)
- Primary commands: `cargo test`, `cargo fmt`, `cargo clippy`
- Entry points: `src/main.rs`, `src/lib.rs`, `src/app.rs`
- Important dirs: `src/fs_ops/`, `src/platform/`, `tests/`
- CI: GitHub Actions matrix (linux, macos, windows)

---

## Hard constraints to include in every AI prompt
Always paste these guardrails into the AI prompt (or reference this file):

- Run the test suite locally and include full test output (`cargo test` or targeted test command).
- Make the smallest possible change needed to solve the request.
- Add or update tests for any behavioral change.
- Preserve public APIs and CLI machine-facing outputs unless explicitly allowed.
- Avoid removing features or tests. If you must remove or change behavior, add a migration note and tests demonstrating the new behavior.
- Do not exfiltrate secrets or call arbitrary external network services.
- If modifying platform-specific code under `src/platform/`, add `#[cfg(...)]` tests or describe the platform impact.

---

## Minimal AI workflow (copy into prompts)
Ask the assistant to follow this exact sequence and return these items in order:

1. One-line preamble: why/what/outcome (1 sentence).
2. Small plan: 2–4 bullet steps of how you'll implement it.
3. Apply patch: provide an `apply_patch`-style diff (or a single patch file) that can be applied to the repo.
4. Run checks: run `cargo fmt` (or `cargo fmt -- --check`), `cargo clippy` (quick), and `cargo test` and paste outputs.
5. If tests fail, iterate (fix only failing tests) and repeat steps 3–4.
6. Summary: list changed files, short explanation, and any follow-up tasks.

---

## Prompt templates (copy/paste and fill placeholders)

Template: Fix failing tests (small scope)

```
Why/What/Outcome: Fix failing tests in the repository; keep changes minimal.

Context: Rust repo (Aria_Move_Rust). Tests run with `cargo test`.

Guardrails: (paste 'Hard constraints' block).

Failing output: <PASTE failing tests here>

Deliverables:
- apply_patch diff that fixes the tests
- run `cargo test --tests` and include output
- short explanation of the fix and tests added/modified
```

Template: Add integration test

```
Why/What/Outcome: Add an integration test that verifies <describe behavior>.

Context: Use `tempfile`/`assert_fs` for isolation; do not touch user files.

Guardrails: (paste 'Hard constraints' block).

Deliverables:
- New test file under `tests/` using appropriate `#[cfg(...)]` if platform-specific
- Run `cargo test --test <file> -- --nocapture` and include output
```

Template: Restore removed feature

```
Why/What/Outcome: Restore feature <name> to match previous behavior.

Context: Describe the expected behavior and where it used to live (files/functions).

Guardrails: (paste 'Hard constraints' block).

Deliverables:
- Minimal patch restoring behavior
- Tests proving parity (fail before, pass after)
- `cargo test` output and short root cause analysis
```

Template: Safe optimization

```
Why/What/Outcome: Improve performance of <module> without changing behavior.

Context: Provide current complexity or runtime scenario.

Deliverables:
- 2–3 alternatives with trade-offs
- Implement the least risky option with tests and a small benchmark or timing harness
- `cargo test` and benchmark output
```

---

## Example full prompt (copy/paste and edit)

```
Why/What/Outcome: Fix failing logging integration test and add a file-path test for macOS.

Context: repo root; tests run with `cargo test`. `src/logging.rs` contains `init_tracing`.

Guardrails: (paste 'Hard constraints' block).

Plan:
- Inspect `src/logging.rs` and the tests under `tests/`.
- Add or update `tests/logging_integration.rs` to cover file logging with a tempdir.
- Run `cargo test --test logging_integration -- --nocapture` and include output.

Deliverables:
- apply_patch diff to repo
- test output
- brief explanation and next steps
```

---

## PR/Commit metadata (copy into reply)
When AI returns a patch, require the following commit message and PR template:

Commit header (one line): `<area>: short description` (max 50 chars)
Commit body: 1–2 sentences why, list tests run, any manual verification steps.

PR description template:
- Title
- Summary
- Testing performed
- Backwards compatibility notes
- Risks and follow-ups

---

## How to use this file in VS Code (quick guide)

1. Open the repository in VS Code.
2. Open `.github/AI_PROMPTS.md` in the editor.
3. Pick a template; replace placeholders (`<...>`) with specifics for your request.
4. Copy the filled prompt to your clipboard.

Using the built-in Chat or an AI extension:
- If you use the GitHub Copilot Chat or ChatGPT extension, paste the prompt into the chat input and ask the assistant to apply changes.
- If you use an external assistant, paste the filled prompt there.

Local verification loop in VS Code:
- Open an integrated terminal (View → Terminal).
- Run a focused test when possible (e.g., `cargo test --test logging_integration -- --nocapture`) to reproduce failures.
- After the assistant returns a patch, apply it using your normal workflow (or the assistant may provide an `apply_patch` format to execute).
- Run `cargo fmt` and `cargo test` in the terminal to verify results.

Recommended VS Code shortcuts to speed this up:
- Open prompts file: Cmd/Ctrl+P → type `.github/AI_PROMPTS.md`
- Open terminal: Ctrl+` (backtick)
- Run last command: Up arrow then Enter in terminal
- Git: use Source Control sidebar to create a branch and commit changes

Optional convenience: create a snippet for VS Code
- Add a user/workspace snippet that inserts your most-used template. Example snippet key: `aiFixTests`.

---

## Troubleshooting & escalation
- If tests are flaky, capture `RUST_BACKTRACE=1` and re-run failing tests 2–3 times.
- If a CI runner behaves differently (e.g., symlinked /tmp), add a test guard to skip in that environment or create test directories under the workspace to avoid symlinked temp parents.
- For platform-specific changes, request human review in the PR.

---

If you want, I can commit this file into the repository now and add a small VS Code snippet file too.
