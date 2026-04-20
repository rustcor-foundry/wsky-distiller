use crate::cli::Format;
use crate::extract::Article;
use chrono::Utc;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use url::Url;

pub fn build_frontmatter(article: &Article, source: Option<&str>) -> String {
    let mut fm = String::new();
    fm.push_str("---\n");
    fm.push_str(&format!("title: {}\n", yaml_quote(&article.title)));
    fm.push_str("author: 'Paul Walker'\n");
    fm.push_str("publisher: 'RustCor Foundry'\n");
    if let Some(src) = source {
        fm.push_str(&format!("source: {}\n", yaml_quote(src)));
    }
    fm.push_str(&format!(
        "fetched_at: {}\n",
        yaml_quote(&Utc::now().to_rfc3339())
    ));
    if let Some(excerpt) = &article.excerpt {
        fm.push_str(&format!("excerpt: {}\n", yaml_quote(excerpt)));
    }
    fm.push_str("---\n\n");
    fm
}

pub fn generate_filename(url: &str, title: &str) -> String {
    let host = Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| "output".to_string());

    let title_slug = sanitize(title);
    let host_slug = sanitize(&host);
    let unique = short_hash(url);
    format!(
        "{}_{}_{}.md",
        host_slug,
        non_empty_slug(&title_slug),
        unique
    )
}

pub fn apply_output_format(markdown: String, format: &Format) -> String {
    match format {
        Format::Rich => markdown,
        Format::Standard => collapse_blank_lines(markdown),
        Format::Minimal => markdown
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn collapse_blank_lines(input: String) -> String {
    let mut output = String::new();
    let mut blank_seen = false;

    for line in input.lines() {
        let blank = line.trim().is_empty();
        if blank {
            if !blank_seen {
                output.push('\n');
            }
            blank_seen = true;
        } else {
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            output.push_str(line);
            output.push('\n');
            blank_seen = false;
        }
    }

    output.trim_end().to_string() + "\n"
}

fn sanitize(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    cleaned
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn yaml_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn short_hash(input: &str) -> String {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())[..8].to_string()
}

fn non_empty_slug(value: &str) -> &str {
    if value.is_empty() {
        "untitled"
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::{build_frontmatter, generate_filename};
    use crate::extract::Article;

    #[test]
    fn filename_includes_stable_hash_for_uniqueness() {
        let first = generate_filename("https://example.com/article?page=1", "Example");
        let second = generate_filename("https://example.com/article?page=2", "Example");

        assert_ne!(first, second);
    }

    #[test]
    fn frontmatter_quotes_yaml_sensitive_content() {
        let article = Article {
            title: "Bob's \"Example\"".to_string(),
            content: String::new(),
            excerpt: Some("It's safe".to_string()),
        };

        let frontmatter = build_frontmatter(&article, Some("https://example.com?q=1"));

        assert!(frontmatter.contains("title: 'Bob''s \"Example\"'"));
        assert!(frontmatter.contains("author: 'Paul Walker'"));
        assert!(frontmatter.contains("publisher: 'RustCor Foundry'"));
        assert!(frontmatter.contains("source: 'https://example.com?q=1'"));
        assert!(frontmatter.contains("excerpt: 'It''s safe'"));
    }
}
