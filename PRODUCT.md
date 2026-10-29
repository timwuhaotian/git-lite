# Product

## Register

product

## Users

Solo developer (primary) and small teams running their own Linux box. They check the UI in spare moments: at the desk to confirm a push landed, or on a phone to see whether a backup made it home. Sessions last seconds, not minutes. They already live in the `gl` CLI; the web UI is a read-only status board, not a workspace.

## Product Purpose

git-lite is an ultra-fast minimal git replacement: save-anytime snapshots plus branch isolation, without git's cost or GitLab's weight. The web UI exists to answer two questions at a glance: what repos exist, and what landed on each branch. Success is a page that loads instantly, needs no build step, and never makes the user think about the tool itself.

## Brand Personality

Blunt, fast, no cruft. Three words: plain, quick, honest. Tone is terse and technical. Nothing decorates; everything informs. The interface should feel like a well-kept lab notebook, not a product landing page.

## Anti-references

- GitLab/GitHub admin chrome: heavy nav rails, tabs within tabs, settings mazes.
- Neon-on-black terminal cosplay and SaaS gradient hero clichés.
- GitHub-dark clone styling that makes every dev tool look identical.
- Dashboards that lead with big-number hero metrics instead of the actual data.

## Design Principles

1. Density over decoration: the log table is the product, give it the pixels.
2. Standard affordances only: sidebar, filter, table. Nothing to learn, nothing to relearn.
3. Zero-build honesty: one static file served by the Rust binary. No framework may break that.
4. Readable anywhere: phone on the subway or monitor at the desk, light or dark room.
5. Silence is a feature: background refresh never flickers, steals focus, or shouts.

## Accessibility & Inclusion

Baseline below WCAG AA effort but concrete: 4.5:1 body text contrast in both themes, visible keyboard focus on every control, no color-only meaning (branches are labeled, status has text), `prefers-reduced-motion` disables transitions, 44px-class tap targets on the repo/branch selectors for phone use.
