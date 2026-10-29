# Contributing to git-lite

Small repo, simple rules.

## Scope

git-lite is intentionally minimal: save-anytime snapshots, branch isolation,
dumb server storage. Before adding anything, check it against the README
non-goals. If it smells like GitLab, it does not belong here.

Preferred contributions: bug fixes, performance wins, missing CLI ergonomics
(`merge`, `diff`), client/server protocol mismatches, docs.

## Workflow

1. Rust 1.80+, edition 2021. `cargo build` and `cargo check` must pass clean.
2. Keep the workspace layout: `gl-core` (shared lib), `gl-server` (binary),
   `gl-client` (binary `gl`), `web/` (single static file, no build step).
3. No new dependencies without a reason stated in the PR. The web UI stays
   dependency-free, framework-free, one file.
4. End-to-end proof for protocol changes: boot `gl-server`, `gl init`,
   `gl save`, `gl push`, fresh-dir `gl pull`, files match.
5. Update README command reference and API table when you change behavior.

## Style

- Error messages say what happened and what to do next.
- CLI output stays script-friendly: one result per line, no spinners.
- No em dashes in user-facing copy.
