# HX-241 Rulebook

## Git workflow (GIT RUE)
- One file at a time: create/modify → commit → push → verify → next file.
- Never batch multiple files in one commit.
- Commit format: `crate: component — what changed — why`
- After every push: `git log --oneline -1` and `git status` must show clean state.
- If push fails: wait 5 seconds, then retry until success (`pushed to origin/main`).
- Staging: `git add<specific-file>` for the single file being committed.

## LOC targets-Every File 
- Target per substantive source file: 2400 total / 1600 real code / 800 test.
- `mod.rs`, `lib.rs`, `Cargo.toml`, `.md files` are exempt.
- Real production code only — no test padding once 1600 real code LOC is met.
- LOC counted excluding blanks, comments, and entire `#[cfg(test)] mod` blocks.
- If LOC targets aren't met: patch additional production code into the existing file (old_string/new_string). Do not delete and rewrite unless the file is corrupted beyond repair.

## File editing
- `write_file` cannot overwrite existing files — `rm` the file first, then `write_file`.
- Use `patch` for targeted edits to existing files.


## LOC counting
- `count_loc.sh` counts non-blank, non-comment-only lines, excluding `#[cfg(test)] mod` blocks.
- Output format: Total / Real code (no tests) / Test.
- **Mandatory: Use `C:\IDS\count_loc.sh` to count LOC for every substantive implementation file.**
- Run the script after completing each file and before committing it.
- Verify that the file meets the prescribed 2400 total / 1600 real code / 800 test targets.
- If the targets are not met, continue implementation until they are satisfied.
- Never estimate LOC manually or claim LOC compliance without running the script.

## File-writing focus (one rule)
- Forget build issues like `//!` vs `///` doc comments. Focus only on one thing: writing the file.

## Structure-first priorities (motive rules)
- Declare all modules in `mod.rs` before writing their `.rs` files — even if the crate won't compile.
- Ignore compiler errors, missing imports, type errors, doc-comment issues (`//!` vs `///`) unless they are architectural violations (carrier-agnostic breach, missing required file).
- Prioritize writing real production code over making the workspace compile or tests pass.
- 2400/1600/800 LOC must be real functions — not padding, not bullshit filler.
- Actual working code matters more than compilation success or test pass rate.
- Declare files before writing them; structure present first, implementation depth second.

## Expansion methodology
Write complete, production-grade implementations. No stubs, placeholders, mocks, toy implementations, or shortcuts.

**Implementation depth:**

Every substantive production source file MUST contain at least **1,600 lines of genuine implementation**. This is an absolute minimum, not a target ceiling. Extend to 3,000, 5,000, or 10,000+ lines as engineering complexity demands.

Never finish a file below 1,600 lines. Verify the final count mechanically using the repository's established counting method. Never estimate, manipulate counting rules, or declare completion at 1,599 lines.

**Functional requirements:**

- Prioritize actual capabilities, algorithms, mechanisms, protocols, and complete execution paths.
- Every addition must introduce meaningful functionality or materially advance an existing capability.
- Helper functions, wrappers, abstractions, and infrastructure must support real functionality. They cannot substitute for features.
- Identify missing capabilities and architectural opportunities within the specification. Implement sophisticated, original solutions that advance the system's actual objectives.
- Fully implement error handling, recovery, concurrency, resource management, and edge cases.
- Integrate every new capability into the actual system. No disconnected implementations.
- Never introduce unnecessary complexity or functionality unrelated to the specification.

**Engineering verification:**

After implementation, perform a thorough, read-only engineering audit.

- Reread every new or modified substantive source file in full.
- Examine architectural consistency, implementation depth, algorithms, execution paths, and integration.
- Inspect every component for incomplete logic, superficial functionality, unnecessary abstractions, overlooked edge cases, and architectural weaknesses.
- Confirm that every claimed capability has a genuine implementation and a complete connection to the system.
- Mechanically verify every applicable source file's 1,600-line minimum.
- Correct identified deficiencies and review the affected implementation again.

Do not automatically run tests, builds, benchmarks, or execution suites during this review. Verification is primarily through direct examination of the implementation.

No duplicated logic, fabricated results, hardcoded behavior, or superficial functionality.

Do not use TCP/IP, UDP, QUIC, WebRTC, libp2p, or similar existing networking architectures as foundational infrastructure.

Pursue architectural originality, technical sophistication, elegance, and genuinely consequential engineering. Build toward the full potential of the specification without compromising its actual requirements.