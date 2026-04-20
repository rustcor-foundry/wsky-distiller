# distill-render

**Headless Chrome renderer with stealth** — companion tool for `distill` that handles JavaScript-heavy sites and outputs final Markdown.

## Features

- Full page rendering with JavaScript execution
- Stealth / anti-detection techniques
- Multiple wait strategies
- Final clean Markdown output through the shared distill core

## Usage

```bash
# Basic
distill-render https://spa.example.com

# With stealth + output file
distill-render https://heavy-app.com --stealth --output final.md
```

## Stealth Techniques Applied

- Removes `navigator.webdriver` flag
- Random User-Agent rotation
- Random viewport size
- Human-like timing

## Integration with distill

```bash
# Recommended workflow for JS-heavy sites
distill-render https://app.com --output clean.md
```

## Requirements

- Will automatically download Chromium on first run
