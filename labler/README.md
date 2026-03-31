# Labler — Cooklang Recipe Validator

A web-based tool for reviewing and correcting `.recipe` → `.cook` conversions side-by-side. Built with Rust (Actix-Web) and the official Cooklang parser.

![Screenshot](static/screenshot.png)

## Features

- Three-panel layout: original `.recipe` source, rendered Cooklang output, and Cooklang source editor
- Syntax-highlighted Cooklang editor (CodeMirror 6)
- Real-time parsing with error/warning display
- Diff view comparing original and rendered output
- Auto-save with dirty state indicator
- Keyboard-driven navigation between recipes

## Usage

```sh
# From the labler/ directory:

cargo run                      # All recipes (defaults to ../recipes)
cargo run -- ../recipes/us     # Only US recipes
cargo run -- ../recipes/gb     # Only GB recipes
```

Then open http://localhost:8080.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `⌘S` | Save |
| `⌘[` | Previous recipe |
| `⌘]` | Next recipe |
| `⌘⌫` | Remove current recipe |

## Validation Workflow

Multiple people can collaborate on reviewing recipes by splitting country folders between them.

### Setup

Each reviewer picks a country folder to work on:

```sh
cargo run -- ../recipes/us     # Reviewer A
cargo run -- ../recipes/gb     # Reviewer B
```

### Review Process

1. Open a recipe and compare the original `.recipe` (left panel) with the Cooklang source (right panel)
2. Fix any conversion issues in the Cooklang editor — the middle panel shows a live preview
3. Use "Show Diff" to spot differences between the original and the rendered output
4. Once the recipe looks correct, add `fine_tune_status: reviewed` to the YAML frontmatter in the `.cook` file:

```cooklang
---
fine_tune_status: reviewed
---

Preheat oven to 425 degrees. Dice @potatoes{12%oz}...
```

5. Save (`⌘S`) and move to the next recipe (`⌘]`)

### Status Tracking

The `fine_tune_status` frontmatter field in `.cook` files tracks review progress:

| Value | Meaning |
|-------|---------|
| *(missing)* | Not yet reviewed |
| `reviewed` | Human-verified and corrected |

This lets the team see at a glance which recipes are done and which still need attention.

## Dependencies

Requires the [cooklang-rs](https://github.com/cooklang/cooklang-rs) parser checked out at `../../cooklang-rs` (i.e. `~/Cooklang/cooklang-rs`).
