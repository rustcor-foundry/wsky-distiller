use anyhow::Result;
use dom_smoothie::Readability;
use regex::Regex;
use scraper::{Html, Selector};

#[derive(Debug, Clone)]
pub struct Article {
    pub title: String,
    pub content: String,
    pub excerpt: Option<String>,
}

pub fn extract_content(html: &str, _source_url: Option<&str>) -> Result<Article> {
    if let Ok(mut readability) = Readability::new(html, _source_url, None) {
        if let Ok(article) = readability.parse() {
            let cleaned = strip_non_content_blocks(article.content.as_ref());
            let text_fallback = text_to_html_paragraphs(&article.text_content);
            let content = if cleaned.trim().is_empty() {
                text_fallback.clone().unwrap_or_default()
            } else {
                cleaned
            };

            if !content.trim().is_empty() {
                return Ok(Article {
                    title: if article.title.trim().is_empty() {
                        "Untitled".to_string()
                    } else {
                        article.title
                    },
                    content,
                    excerpt: article.excerpt.or_else(|| {
                        excerpt_from_text(&article.text_content)
                    }),
                });
            }
        }
    }

    let document = Html::parse_document(html);

    let title_selector = Selector::parse("title").expect("valid selector");
    let candidate_selector = Selector::parse(
        "article, main, [role=\"main\"], .post-content, .entry-content, .article, body",
    )
    .expect("valid selector");

    let title = document
        .select(&title_selector)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Untitled".to_string());

    let content = document
        .select(&candidate_selector)
        .map(|node| {
            let html = node.inner_html();
            let text = node
                .text()
                .collect::<Vec<_>>()
                .join(" ");
            let text_len = text
                .split_whitespace()
                .map(str::len)
                .sum::<usize>();
            (html, text, text_len)
        })
        .max_by_key(|(_, _, text_len)| *text_len)
        .map(|(candidate_html, candidate_text, _)| {
            let cleaned = strip_non_content_blocks(&candidate_html);
            if cleaned.trim().is_empty() {
                text_to_html_paragraphs(&candidate_text).unwrap_or_default()
            } else {
                cleaned
            }
        })
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| {
            let cleaned = strip_non_content_blocks(html);
            if cleaned.trim().is_empty() {
                text_to_html_paragraphs(&document.root_element().text().collect::<Vec<_>>().join(" "))
                    .unwrap_or_default()
            } else {
                cleaned
            }
        });

    let excerpt_text = html_to_text(&content);
    let excerpt = excerpt_text
        .split_whitespace()
        .take(40)
        .collect::<Vec<_>>()
        .join(" ");

    Ok(Article {
        title,
        content,
        excerpt: if excerpt.is_empty() {
            None
        } else {
            Some(excerpt)
        },
    })
}

fn strip_non_content_blocks(html: &str) -> String {
    let mut cleaned = html.to_string();

    for pattern in [
        r"(?is)<script\b[^>]*>.*?</script>",
        r"(?is)<style\b[^>]*>.*?</style>",
        r"(?is)<noscript\b[^>]*>.*?</noscript>",
        r"(?is)<svg\b[^>]*>.*?</svg>",
        r"(?is)<nav\b[^>]*>.*?</nav>",
        r"(?is)<header\b[^>]*>.*?</header>",
        r"(?is)<footer\b[^>]*>.*?</footer>",
        r"(?is)<aside\b[^>]*>.*?</aside>",
        r"(?is)<form\b[^>]*>.*?</form>",
    ] {
        let re = Regex::new(pattern).expect("valid cleanup regex");
        cleaned = re.replace_all(&cleaned, "").to_string();
    }

    cleaned
}

fn html_to_text(html: &str) -> String {
    let tag_re = Regex::new(r"(?is)<[^>]+>").expect("valid tag regex");
    tag_re.replace_all(html, " ").into_owned()
}

fn excerpt_from_text(text: &str) -> Option<String> {
    let excerpt = text
        .split_whitespace()
        .take(40)
        .collect::<Vec<_>>()
        .join(" ");
    if excerpt.is_empty() {
        None
    } else {
        Some(excerpt)
    }
}

fn text_to_html_paragraphs(text: &str) -> Option<String> {
    let text = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if text.trim().is_empty() {
        return None;
    }

    let paragraphs = text
        .split('\n')
        .map(|line| format!("<p>{}</p>", line.trim()))
        .collect::<Vec<_>>()
        .join("\n");

    Some(paragraphs)
}

#[cfg(test)]
mod tests {
    use super::extract_content;

    #[test]
    fn prefers_article_content_over_shell_markup() {
        let html = r#"
            <html>
                <head><title>Example</title></head>
                <body>
                    <nav>Navigation</nav>
                    <article><h1>Headline</h1><p>Main story text.</p></article>
                    <footer>Footer links</footer>
                </body>
            </html>
        "#;

        let article = extract_content(html, Some("https://example.com")).unwrap();

        assert_eq!(article.title, "Example");
        assert!(article.content.contains("Headline"));
        assert!(!article.content.contains("Navigation"));
        assert!(!article.content.contains("Footer links"));
    }

    #[test]
    fn falls_back_to_text_paragraphs_when_cleaned_html_is_empty() {
        let html = r#"
            <html>
                <head><title>Example</title></head>
                <body>
                    Plain intro text.
                    <script>console.log('ignored')</script>
                    More important body copy.
                </body>
            </html>
        "#;

        let article = extract_content(html, Some("https://example.com")).unwrap();

        assert!(article.content.contains("Plain intro text."));
        assert!(article.content.contains("More important body copy."));
    }
}
