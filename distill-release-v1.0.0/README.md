# distill v1.0.0 — Release

**The complete ecosystem for converting websites to clean Markdown for AI consumption.**

## What's Included

- `distill` — Lightweight, fast HTTP distiller (pure Rust)
- `distill-render` — Headless Chrome renderer for JS-heavy sites that outputs final Markdown
- `distill-tui` — Cross-platform terminal UI for guided runs without any page preview
- `distill-gui` — "Norton Commander" style desktop GUI with rich export options

## Quick Start

### Linux / macOS

```bash
chmod +x install.sh
./install.sh
```

### Windows

```powershell
.\install.ps1
```

## Features

- Parallel batch processing with rate limiting
- `robots.txt` respect + proxy rotation (HTTP + SOCKS5)
- Advanced stealth rendering for SPAs
- Cross-platform terminal workflow for servers and basic machines
- Queue-based desktop GUI workflow
- Export formats: ZIP, combined Markdown, clipboard copy, individual files
- Local HTML file or stdin ingestion for CLI workflows

## Documentation

See `docs/` folder for detailed guides.

## License

MIT
