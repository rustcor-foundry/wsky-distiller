# distill

**Fast, ethical webpage → clean Markdown** for AI consumption.

The shared extraction and conversion engine is exposed as the `distill_core` library crate.

## Features

- Readability-first content extraction with fallback heuristics
- Clean, structured Markdown with rich frontmatter
- Parallel batch processing with rate limiting
- `robots.txt` respect
- HTTP + SOCKS5 proxy rotation
- Local HTML file input or `--stdin` pipelines
- Optional fast mode (`--fast`) when built with the `fast` feature
- Retry logic with exponential backoff

## Installation

```bash
cargo install --git https://github.com/yourname/distill
```

## Usage

### Single URL
```bash
distill https://example.com/article -o article.md
```

### Batch Mode (Recommended)
```bash
distill --batch urls.txt --output-dir ./distilled --delay 750 --concurrency 6
```

### With Proxies
```bash
distill --batch urls.txt --proxies proxies.txt --delay 800
```

### Fast Mode
```bash
cargo build --release --features fast
distill https://site.com --fast
```

### From a Saved HTML File
```bash
distill rendered.html -o clean.md
```

## CLI Reference

| Flag                  | Default   | Description |
|-----------------------|-----------|-----------|
| `--batch <file>`      | -         | Process multiple URLs |
| `--output-dir <dir>`  | `distilled` | Output directory |
| `--delay <ms>`        | 500       | Delay between requests |
| `--concurrency <n>`   | 8         | Max parallel requests |
| `--respect-robots`    | true      | Respect robots.txt |
| `--proxies <file>`    | -         | Proxy rotation file |
| `--fast`              | false     | Use fast converter when built with `--features fast` |
| `--dry-run`           | false     | Preview without writing |

## Ethical Guidelines

- Always respect `robots.txt`
- Use reasonable delays (`--delay 500+`)
- Limit concurrency on small sites
- Use proxies when scraping aggressively

## License

MIT
