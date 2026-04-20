use anyhow::Result;
use html_to_markdown_rs::convert as convert_html;
use regex::Regex;

pub fn convert_to_markdown(html: &str, include_images: bool, fast: bool) -> Result<String> {
    #[cfg(not(feature = "fast"))]
    if fast {
        anyhow::bail!("--fast requires rebuilding distill with --features fast");
    }

    let mut markdown = convert_html(html, None)?.content.unwrap_or_default();

    if !include_images {
        let image_re = Regex::new(r"!\[[^\]]*\]\([^)]*\)")?;
        markdown = image_re.replace_all(&markdown, "").to_string();
    }

    if markdown.trim().is_empty() {
        markdown = html_to_plain_markdown(html);
    }

    Ok(cleanup_markdown(markdown)?)
}

fn html_to_plain_markdown(html: &str) -> String {
    let block_re = Regex::new(r"(?i)</?(p|div|section|article|main|li|h[1-6]|br)\b[^>]*>").expect("valid block regex");
    let tag_re = Regex::new(r"(?is)<[^>]+>").expect("valid tag regex");

    let with_breaks = block_re.replace_all(html, "\n");
    tag_re
        .replace_all(&with_breaks, " ")
        .split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn cleanup_markdown(markdown: String) -> Result<String> {
    let timestamp_re = Regex::new(r"^\d{1,2}:\d{2}$")?;
    let editor_mode_re = Regex::new(r"^--\s*(INSERT|NORMAL|VISUAL|REPLACE)\s*--.*$")?;
    let standalone_number_re = Regex::new(r"^\d{1,3}$")?;
    let filename_re = Regex::new(r"^[\w./-]+\.(rs|ts|tsx|js|jsx|py|sh|go|java|c|cpp|h|hpp|rb)$")?;
    let multiline_blank_re = Regex::new(r"\n{3,}")?;

    let normalized = markdown.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let mut cleaned = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index].trim_end();
        let trimmed = line.trim();

        if editor_mode_re.is_match(trimmed) || timestamp_re.is_match(trimmed) {
            index += 1;
            continue;
        }

        if filename_re.is_match(trimmed) {
            let next = lines.get(index + 1).map(|value| value.trim()).unwrap_or("");
            let next_next = lines.get(index + 2).map(|value| value.trim()).unwrap_or("");
            let (number_run_len, _) = standalone_number_run(&lines, index + 1, &standalone_number_re);
            if timestamp_re.is_match(next)
                || standalone_number_re.is_match(next)
                || standalone_number_re.is_match(next_next)
                || number_run_len >= 3
            {
                index += 1;
                continue;
            }
        }

        if standalone_number_re.is_match(trimmed) {
            let (number_run_len, next_index) = standalone_number_run(&lines, index, &standalone_number_re);
            if number_run_len >= 3 {
                index = next_index;
                continue;
            }
        }

        if trimmed.is_empty() {
            let (number_run_len, next_index) = standalone_number_run(&lines, index, &standalone_number_re);
            if number_run_len >= 3 {
                index = next_index;
                continue;
            }
        }

        cleaned.push(trimmed.to_string());
        index += 1;
    }

    let output = multiline_blank_re
        .replace_all(cleaned.join("\n").trim(), "\n\n")
        .to_string();

    Ok(output.trim().to_string() + "\n")
}

fn standalone_number_run(lines: &[&str], start: usize, number_re: &Regex) -> (usize, usize) {
    let mut index = start;
    let mut count = 0;
    let mut saw_content = false;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }

        if number_re.is_match(trimmed) {
            saw_content = true;
            count += 1;
            index += 1;
            continue;
        }

        break;
    }

    if saw_content {
        (count, index)
    } else {
        (0, start)
    }
}

#[cfg(test)]
mod tests {
    use super::convert_to_markdown;

    #[test]
    fn keeps_text_when_converter_would_otherwise_return_empty() {
        let markdown = convert_to_markdown(
            "<div>Alpha</div><div>Beta</div>",
            false,
            false,
        )
        .unwrap();

        assert!(markdown.contains("Alpha"));
        assert!(markdown.contains("Beta"));
    }

    #[test]
    fn removes_editor_chrome_noise_from_markdown() {
        let markdown = convert_to_markdown(
            "<div>mission_statement.rs</div><div>12:42</div><div>1</div><div>2</div><div>3</div><div>4</div><div>pub fn mission() {}</div><div>-- INSERT --</div>",
            false,
            false,
        )
        .unwrap();

        assert!(!markdown.contains("12:42"));
        assert!(!markdown.contains("-- INSERT --"));
        assert!(!markdown.contains("\n1\n2\n3\n4\n"));
        assert!(markdown.contains("pub fn mission() {}"));
    }

    #[test]
    fn removes_number_gutters_even_when_blank_lines_are_interleaved() {
        let markdown = convert_to_markdown(
            "<div>mission_statement.rs</div><div>1</div><div></div><div>2</div><div></div><div>3</div><div></div><div>4</div><div>pub fn mission() {}</div>",
            false,
            false,
        )
        .unwrap();

        assert!(!markdown.contains("mission_statement.rs"));
        assert!(!markdown.contains("\n1\n"));
        assert!(!markdown.contains("\n2\n"));
        assert!(markdown.contains("pub fn mission() {}"));
    }
}
