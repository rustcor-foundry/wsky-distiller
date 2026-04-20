pub mod batch;
pub mod cli;
pub mod convert;
pub mod extract;
pub mod fetch;
pub mod proxy;
pub mod robots;
pub mod utils;

use anyhow::Result;
pub use cli::Format;
pub use extract::Article;

#[derive(Clone, Debug)]
pub struct DistilledDocument {
    pub article: Article,
    pub markdown: String,
}

#[derive(Clone, Debug)]
pub struct DistillOptions {
    pub include_images: bool,
    pub no_frontmatter: bool,
    pub format: Format,
    pub fast: bool,
}

impl Default for DistillOptions {
    fn default() -> Self {
        Self {
            include_images: false,
            no_frontmatter: false,
            format: Format::Rich,
            fast: false,
        }
    }
}

pub fn markdown_from_html(
    html: &str,
    source: Option<&str>,
    options: &DistillOptions,
) -> Result<String> {
    Ok(distill_html(html, source, options)?.markdown)
}

pub fn distill_html(
    html: &str,
    source: Option<&str>,
    options: &DistillOptions,
) -> Result<DistilledDocument> {
    let article = extract::extract_content(html, source)?;
    let mut md =
        convert::convert_to_markdown(&article.content, options.include_images, options.fast)?;
    md = utils::apply_output_format(md, &options.format);

    if !options.no_frontmatter {
        let frontmatter = utils::build_frontmatter(&article, source);
        md = format!("{}{}", frontmatter, md);
    }

    Ok(DistilledDocument {
        article,
        markdown: md,
    })
}

pub fn is_low_content_markdown(markdown: &str) -> bool {
    let body = strip_frontmatter(markdown).trim();
    if body.is_empty() {
        return true;
    }

    let visible_chars = body.chars().filter(|c| !c.is_whitespace()).count();
    let non_empty_lines = body.lines().filter(|line| !line.trim().is_empty()).count();

    visible_chars < 160 && non_empty_lines < 3
}

fn strip_frontmatter(markdown: &str) -> &str {
    if let Some(rest) = markdown.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return &rest[end + 5..];
        }
    }
    markdown
}

#[cfg(test)]
mod tests {
    use super::{is_low_content_markdown, markdown_from_html, DistillOptions, Format};

    #[test]
    fn shared_core_returns_markdown_with_frontmatter() {
        let markdown = markdown_from_html(
            "<html><head><title>Example</title></head><body><article><h1>Hello</h1><p>World</p></article></body></html>",
            Some("https://example.com/post"),
            &DistillOptions {
                include_images: false,
                no_frontmatter: false,
                format: Format::Rich,
                fast: false,
            },
        )
        .unwrap();

        assert!(markdown.contains("title: 'Example'"));
        assert!(markdown.contains("source: 'https://example.com/post'"));
        assert!(markdown.contains("Hello"));
        assert!(markdown.contains("World"));
    }

    #[test]
    fn low_content_detector_flags_frontmatter_only_output() {
        assert!(is_low_content_markdown("---\ntitle: 'Example'\n---\n\n"));
        assert!(!is_low_content_markdown(
            "---\ntitle: 'Example'\n---\n\n# Example\n\nThis is real content with enough substance to count as a meaningful extraction result for the quality guard.\n\n## Details\n\nAnother non-empty section keeps the line count and body length above the low-content threshold.\n"
        ));
    }
}
