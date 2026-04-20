# Quick Start Guide

## 1. Single URL

```bash
distill https://example.com/article -o article.md
```

## 2. Batch Mode (Recommended)

```bash
distill --batch urls.txt --output-dir ./distilled --delay 750 --concurrency 6
```

## 3. JS-Heavy Sites

```bash
distill-render https://spa-site.com -o clean.md
```

Or use the main CLI with pre-rendered HTML if you already saved it:

```bash
distill rendered.html -o clean.md
```

## 4. Using the GUI

```bash
distill-gui
```

- Add URLs manually or upload file
- Set per-job options
- Click Start
- Export in multiple formats

## 5. Using the TUI

```bash
distill-tui
```

- Run single URL or batch jobs from any terminal
- Choose standard or render mode
- Export individual files, `combined.md`, or `distilled.zip`
- No GPU or page preview required
