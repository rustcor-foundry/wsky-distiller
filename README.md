# wsky-distiller

Whiskey Distiller is a small Rust toolkit for turning webpages into clean Markdown.

## Components

- `distill` - main CLI for standard fetch-and-convert flows
- `distill-render` - JS-heavy site helper that renders and then emits final Markdown
- `distill-tui` - cross-platform terminal UI for guided runs, batch imports, and exports
- `distill-gui` - richer desktop workflow for graphics-capable machines

## Recommended Flow

- Use `distill` for ordinary pages
- Use `distill-render` for JS-heavy pages
- Use `distill-tui` when you want a lightweight interface without a desktop GUI

## Repo Notes

- Shared extraction and conversion logic lives in `distill` as the `distill_core` library target
- `distill-render` depends on that shared core and outputs final Markdown
- `distill-tui` is designed to work well on Windows, macOS, Linux, and basic terminal environments

## Status

The current workspace has been QA-polished around:

- standard URL-to-Markdown conversion
- JS-heavy fallback rendering
- batch processing and export support
- a branded TUI with safer navigation and a persistent output pane

See `distill-release-v1.0.0/docs/QUICKSTART.md` for the current quick start.
