use anyhow::{anyhow, Context, Result};
use console::{style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use distill_core::fetch::fetch_with_retry;
use distill_core::robots::RobotsChecker;
use distill_core::utils::generate_filename;
use distill_core::{distill_html, is_low_content_markdown, DistillOptions, Format};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use zip::write::FileOptions;

fn main() -> Result<()> {
    let theme = ColorfulTheme::default();
    let term = Term::stdout();
    let mut config = AppConfig::default();
    let mut state = AppState::default();

    loop {
        clear_screen(&term)?;
        render_dashboard(&config, &state);

        let choice = Select::with_theme(&theme)
            .with_prompt("Command")
            .items(&[
                "Fetch   Single URL",
                "Batch   Import URL file",
                "Config  Output and conversion settings",
                "Export  Combined Markdown",
                "Export  ZIP archive",
                "Files   Show output files",
                "Exit    Quit",
            ])
            .default(0)
            .interact_opt()?;

        let result = match choice {
            Some(0) => run_single_url(&theme, &config),
            Some(1) => run_batch_file(&theme, &config),
            Some(2) => configure_settings(&theme, &mut config),
            Some(3) => export_combined(&config.output_dir)
                .map(|path| vec![format!("Combined Markdown written -> {}", clickable_path(&path))]),
            Some(4) => export_zip(&config.output_dir)
                .map(|path| vec![format!("ZIP archive written -> {}", clickable_path(&path))]),
            Some(5) => show_output_files(&theme, &config.output_dir),
            Some(6) | None => break,
            _ => unreachable!(),
        };

        match result {
            Ok(lines) => {
                if let Some(last) = lines.last() {
                    state.set_status(last.clone());
                } else {
                    state.set_status("Ready");
                }
                for line in lines {
                    state.push(line);
                }
            }
            Err(err) => {
                state.set_status(format!("ERROR {err}"));
                state.push(format!("ERROR {err}"));
            }
        }
    }

    Ok(())
}

#[derive(Default)]
struct AppState {
    messages: Vec<String>,
    status: String,
}

impl AppState {
    fn push(&mut self, message: String) {
        self.messages.push(message);
        if self.messages.len() > 14 {
            let drop_count = self.messages.len() - 14;
            self.messages.drain(0..drop_count);
        }
    }

    fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }
}

#[derive(Clone)]
struct AppConfig {
    output_dir: PathBuf,
    render_mode: bool,
    include_images: bool,
    no_frontmatter: bool,
    format: Format,
    respect_robots: bool,
    delay_ms: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            render_mode: false,
            include_images: false,
            no_frontmatter: false,
            format: Format::Rich,
            respect_robots: true,
            delay_ms: 500,
        }
    }
}

fn default_output_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("distill-output")
}

fn render_dashboard(config: &AppConfig, state: &AppState) {
    println!();
    println!("{}", style("+-[ WSKY DISTILLER ]---------------------------------------------+").color256(208).bold());
    println!(
        "{}",
        style("|        __                                                    |")
            .color256(208)
    );
    println!(
        "{}",
        style("|       |  |                                                   |")
            .color256(208)
    );
    println!(
        "{}",
        style("|     __|  |__ Distill URLs into clean Markdown from any       |")
            .color256(208)
    );
    println!(
        "{}",
        style("|    /  _  _  \\ terminal. Batch-friendly. Render-capable.      |")
            .color256(208)
    );
    println!(
        "{}",
        style("|   |  |_| |_|  | Operator-focused.                            |")
            .color256(208)
    );
    println!(
        "{}",
        style("|   |         _ |                                              |")
            .color256(208)
    );
    println!(
        "{}",
        style("|   |  X X X | |                                              |")
            .color256(208)
    );
    println!(
        "{}",
        style("|   |________|_/                                              |")
            .color256(208)
    );
    println!("{}", style("+-[ CONTROL DECK ]-----------------------------------------------+").dim());
    render_split_panes(config, state);
    println!("{}", style("+-[ OUTPUT ]-----------------------------------------------------+").dim());
    if state.messages.is_empty() {
        println!(
            "{}",
            style("  No recent output yet. Run a fetch, export, or file action to populate this pane.")
                .dim()
        );
    } else {
        for line in &state.messages {
            println!("  {}", style(line).white());
        }
    }
    println!("{}", style("+-[ STATUS ]-----------------------------------------------------+").dim());
    let status = if state.status.trim().is_empty() {
        "Ready"
    } else {
        state.status.as_str()
    };
    println!("  {}", style(status).color256(214).bold());
}

fn render_split_panes(config: &AppConfig, _state: &AppState) {
    let left = vec![
        format!("Output: {}", format_path(&config.output_dir)),
        format!(
            "Mode: {}   Format: {}   Delay: {}ms",
            mode_label(config.render_mode),
            format_label(&config.format),
            config.delay_ms
        ),
        format!(
            "Images: {}   Frontmatter: {}   Robots: {}",
            on_off(config.include_images),
            on_off(!config.no_frontmatter),
            on_off(config.respect_robots)
        ),
        String::new(),
        "Commands".to_string(),
        "  Fetch   Single URL".to_string(),
        "  Batch   Import URL file".to_string(),
        "  Config  Output and conversion settings".to_string(),
        "  Export  Combined Markdown / ZIP".to_string(),
        "  Files   Browse saved Markdown".to_string(),
        String::new(),
        "Keys: arrows/j-k move  enter select".to_string(),
        "      esc back/exit  blank input cancels".to_string(),
    ];

    let recent_files = recent_file_lines(&config.output_dir);
    let mut right = vec!["Recent Files".to_string()];
    if recent_files.is_empty() {
        right.push("  No Markdown files in output directory".to_string());
    } else {
        right.extend(recent_files);
    }

    let rows = left.len().max(right.len());
    for index in 0..rows {
        let left_text = left.get(index).map_or("", String::as_str);
        let right_text = right.get(index).map_or("", String::as_str);
        println!(
            "{} {} {}",
            style(format!("| {:<34}", truncate_display(left_text, 34))).white(),
            style("|").dim(),
            style(format!(" {:<33} |", truncate_display(right_text, 33))).white()
        );
    }
}

fn recent_file_lines(dir: &Path) -> Vec<String> {
    let mut files = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            })
            .collect::<Vec<_>>(),
        Err(_) => return Vec::new(),
    };

    files.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
    });
    files.reverse();

    files
        .into_iter()
        .take(8)
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("output.md");
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            format!("  {} ({}b)", name, size)
        })
        .collect()
}

fn truncate_display(input: &str, width: usize) -> String {
    let mut output = String::new();
    for ch in input.chars().take(width) {
        output.push(ch);
    }
    if input.chars().count() > width && width > 1 {
        output.pop();
        output.push('>');
    }
    output
}

fn configure_settings(theme: &ColorfulTheme, config: &mut AppConfig) -> Result<Vec<String>> {
    let mut messages = Vec::new();
    loop {
        let choice = Select::with_theme(theme)
            .with_prompt("Choose a setting")
            .items(&[
                format!("Output dir   {}", config.output_dir.display()),
                format!("Mode         {}", mode_label(config.render_mode)),
                format!("Images       {}", on_off(config.include_images)),
                format!("Frontmatter  {}", on_off(!config.no_frontmatter)),
                format!("Robots       {}", on_off(config.respect_robots)),
                format!("Delay        {}ms", config.delay_ms),
                format!("Format       {}", format_label(&config.format)),
                "Back".to_string(),
            ])
            .default(0)
            .interact_opt()?;

        match choice {
            Some(0) => {
                if let Some(output_dir) = prompt_text(theme, "Output directory", &config.output_dir.display().to_string())? {
                    config.output_dir = PathBuf::from(output_dir);
                    messages.push(format!(
                        "Output directory set to {}",
                        clickable_path(&config.output_dir)
                    ));
                }
            }
            Some(1) => {
                if let Some(value) = confirm_opt(theme, "Use distill-render for JS-heavy sites?", config.render_mode)? {
                    config.render_mode = value;
                    messages.push(format!("Mode set to {}", mode_label(config.render_mode)));
                }
            }
            Some(2) => {
                if let Some(value) = confirm_opt(theme, "Include images in Markdown?", config.include_images)? {
                    config.include_images = value;
                    messages.push(format!("Images {}", on_off(config.include_images)));
                }
            }
            Some(3) => {
                if let Some(value) = confirm_opt(theme, "Include frontmatter?", !config.no_frontmatter)? {
                    config.no_frontmatter = !value;
                    messages.push(format!("Frontmatter {}", on_off(!config.no_frontmatter)));
                }
            }
            Some(4) => {
                if let Some(value) = confirm_opt(theme, "Respect robots.txt?", config.respect_robots)? {
                    config.respect_robots = value;
                    messages.push(format!("Robots {}", on_off(config.respect_robots)));
                }
            }
            Some(5) => {
                if let Some(value) = prompt_text(theme, "Delay between batch requests (ms)", &config.delay_ms.to_string())? {
                    if let Ok(delay) = value.parse::<u64>() {
                        config.delay_ms = delay;
                        messages.push(format!("Delay set to {}ms", config.delay_ms));
                    } else {
                        messages.push("Invalid delay value; keeping current setting".to_string());
                    }
                }
            }
            Some(6) => {
                if let Some(format_index) = Select::with_theme(theme)
                    .with_prompt("Output format")
                    .items(&["Rich", "Standard", "Minimal"])
                    .default(match config.format {
                        Format::Rich => 0,
                        Format::Standard => 1,
                        Format::Minimal => 2,
                    })
                    .interact_opt()? {
                    config.format = match format_index {
                        0 => Format::Rich,
                        1 => Format::Standard,
                        2 => Format::Minimal,
                        _ => unreachable!(),
                    };
                    messages.push(format!("Format set to {}", format_label(&config.format)));
                }
            }
            Some(7) | None => break,
            _ => unreachable!(),
        }
    }

    Ok(messages)
}

fn run_single_url(theme: &ColorfulTheme, config: &AppConfig) -> Result<Vec<String>> {
    let Some(url) = prompt_text(theme, "URL", "")? else {
        return Ok(vec!["Single URL fetch canceled".to_string()]);
    };
    run_jobs(vec![url], config)
}

fn run_batch_file(theme: &ColorfulTheme, config: &AppConfig) -> Result<Vec<String>> {
    let Some(path) = prompt_text(theme, "Batch file path", "")? else {
        return Ok(vec!["Batch import canceled".to_string()]);
    };
    let urls = read_batch_file(Path::new(&path))?;
    run_jobs(urls, config)
}

fn run_jobs(urls: Vec<String>, config: &AppConfig) -> Result<Vec<String>> {
    if urls.is_empty() {
        return Err(anyhow!("No URLs provided"));
    }

    fs::create_dir_all(&config.output_dir)?;

    let pb = ProgressBar::new(urls.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )?
        .progress_chars("=> "),
    );

    let robots_checker = if config.respect_robots && !config.render_mode {
        Some(RobotsChecker::new(
            "Mozilla/5.0 (compatible; distill/1.0)",
            "distill",
        ))
    } else {
        None
    };

    let mut successes = Vec::new();
    let mut failures = Vec::new();

    for (index, url) in urls.iter().enumerate() {
        pb.set_message(url.clone());
        let result = if config.render_mode {
            run_render_job(url, config)
        } else {
            run_standard_job(url, config, robots_checker.as_ref())
        };

        match result {
            Ok(path) => successes.push(path),
            Err(err) => failures.push(format!("{url} - {err}")),
        }

        pb.inc(1);
        if config.delay_ms > 0 && index + 1 < urls.len() {
            thread::sleep(Duration::from_millis(config.delay_ms));
        }
    }

    pb.finish_and_clear();

    let mut lines = vec![format!(
        "Completed {} successful, {} failed",
        successes.len(),
        failures.len()
    )];
    lines.push(format!(
        "Output directory {}",
        clickable_path(&config.output_dir)
    ));

    for path in &successes {
        lines.push(format!("OK {}", clickable_path(path)));
    }
    for failure in &failures {
        lines.push(format!("FAIL {}", failure));
    }

    Ok(lines)
}

fn run_standard_job(
    url: &str,
    config: &AppConfig,
    robots_checker: Option<&RobotsChecker>,
) -> Result<PathBuf> {
    if let Some(checker) = robots_checker {
        if !checker.is_allowed(url) {
            return Err(anyhow!("Blocked by robots.txt"));
        }
    }

    let html = fetch_with_retry(url, None, 3)?;
    let distilled = distill_html(
        &html,
        Some(url),
        &DistillOptions {
            include_images: config.include_images,
            no_frontmatter: config.no_frontmatter,
            format: config.format.clone(),
            fast: false,
        },
    )?;

    let path = config
        .output_dir
        .join(generate_filename(url, &distilled.article.title));

    if is_low_content_markdown(&distilled.markdown) {
        if resolve_render_executable().is_ok() {
            return run_render_job(url, config);
        }
        return Err(anyhow!(
            "Low-content result detected; this site likely needs render mode"
        ));
    }

    fs::write(&path, distilled.markdown)?;
    Ok(path)
}

fn run_render_job(url: &str, config: &AppConfig) -> Result<PathBuf> {
    let output_path = config.output_dir.join(generate_filename(url, "rendered"));
    let render_exe = resolve_render_executable()?;

    let mut command = Command::new(render_exe);
    command.arg(url).arg("-o").arg(&output_path);

    if config.include_images {
        command.arg("--include-images");
    }
    if config.no_frontmatter {
        command.arg("--no-frontmatter");
    }
    if matches!(config.format, Format::Standard) {
        command.arg("--format").arg("standard");
    }
    if matches!(config.format, Format::Minimal) {
        command.arg("--format").arg("minimal");
    }

    let output = command.output().context("failed to start distill-render")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if !stderr.is_empty() { stderr } else { stdout };
        return Err(anyhow!(message));
    }

    Ok(output_path)
}

fn resolve_render_executable() -> Result<PathBuf> {
    let exe_name = if cfg!(windows) {
        "distill-render.exe"
    } else {
        "distill-render"
    };

    let current_exe = std::env::current_exe()?;
    let current_dir = current_exe
        .parent()
        .ok_or_else(|| anyhow!("Cannot resolve current executable directory"))?;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .ok_or_else(|| anyhow!("Cannot resolve workspace root"))?;

    let candidates = [
        current_dir.join(exe_name),
        workspace_root
            .join("distill-render")
            .join("target")
            .join("release")
            .join(exe_name),
        workspace_root
            .join("distill-render")
            .join("target")
            .join("debug")
            .join(exe_name),
    ];

    candidates
        .into_iter()
        .find(|path| path.is_file())
        .or_else(|| Some(PathBuf::from(exe_name)))
        .ok_or_else(|| anyhow!("Could not find distill-render executable"))
}

fn read_batch_file(path: &Path) -> Result<Vec<String>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed reading batch file {}", path.display()))?;

    let urls = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if urls.is_empty() {
        return Err(anyhow!("No URLs found in batch file"));
    }

    Ok(urls)
}

fn show_output_files(theme: &ColorfulTheme, dir: &Path) -> Result<Vec<String>> {
    let files = collect_markdown_files(dir)?;
    if files.is_empty() {
        return Ok(vec![format!(
            "No Markdown files found in {}",
            clickable_path(dir)
        )]);
    }

    let mut messages = vec![format!("Browsing output files in {}", clickable_path(dir))];
    let file_items = files
        .iter()
        .map(|path| {
            let label = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("output.md");
            let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            format!("{label}   {} bytes", size)
        })
        .collect::<Vec<_>>();

    loop {
        let mut items = file_items.clone();
        items.push("Back".to_string());

        let choice = Select::with_theme(theme)
            .with_prompt("Output files")
            .items(&items)
            .default(0)
            .interact_opt()?;

        match choice {
            Some(index) if index < files.len() => {
                messages.push(format!("FILE {}", clickable_path(&files[index])));
            }
            Some(_) | None => break,
        }
    }

    Ok(messages)
}

fn collect_markdown_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("cannot read output folder {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|value| value.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("combined.md"))
        .collect();

    files.sort();
    Ok(files)
}

fn export_combined(dir: &Path) -> Result<PathBuf> {
    let files = collect_markdown_files(dir)?;
    if files.is_empty() {
        return Err(anyhow!("No Markdown files found"));
    }

    let mut combined = String::new();
    for path in files {
        let content = fs::read_to_string(&path)?;
        combined.push_str(&format!("\n\n<!-- {} -->\n\n", path.display()));
        combined.push_str(&content);
    }

    let target = dir.join("combined.md");
    fs::write(&target, combined)?;
    Ok(target)
}

fn export_zip(dir: &Path) -> Result<PathBuf> {
    let files = collect_markdown_files(dir)?;
    if files.is_empty() {
        return Err(anyhow!("No Markdown files found"));
    }

    let target = dir.join("distilled.zip");
    let file = fs::File::create(&target)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for path in files {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("Invalid UTF-8 filename"))?;
        zip.start_file(name, options)?;
        let content = fs::read(&path)?;
        zip.write_all(&content)?;
    }

    zip.finish()?;
    Ok(target)
}

fn format_label(format: &Format) -> &'static str {
    match format {
        Format::Rich => "rich",
        Format::Standard => "standard",
        Format::Minimal => "minimal",
    }
}

fn mode_label(render_mode: bool) -> &'static str {
    if render_mode {
        "render"
    } else {
        "standard"
    }
}

fn on_off(enabled: bool) -> &'static str {
    if enabled { "on" } else { "off" }
}

fn prompt_text(theme: &ColorfulTheme, prompt: &str, default: &str) -> Result<Option<String>> {
    let prompt_label = if default.trim().is_empty() {
        format!("{prompt} (leave blank to cancel)")
    } else {
        format!("{prompt} (current: {default}; leave blank to cancel)")
    };

    let value: String = Input::with_theme(theme)
        .with_prompt(prompt_label)
        .allow_empty(true)
        .interact_text()?;

    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed))
    }
}

fn confirm_opt(theme: &ColorfulTheme, prompt: &str, default: bool) -> Result<Option<bool>> {
    Confirm::with_theme(theme)
        .with_prompt(prompt)
        .default(default)
        .interact_opt()
        .map_err(Into::into)
}

fn format_path(path: &Path) -> String {
    path.display().to_string()
}

fn clickable_path(path: &Path) -> String {
    let label = format_path(path);
    terminal_link(&label, path)
}

fn terminal_link(label: &str, path: &Path) -> String {
    let target = format!("file:///{}", path.to_string_lossy().replace('\\', "/"));
    format!("\x1b]8;;{target}\x1b\\{label}\x1b]8;;\x1b\\")
}

fn clear_screen(term: &Term) -> Result<()> {
    term.clear_screen()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        collect_markdown_files, export_combined, export_zip, read_batch_file,
    };
    use distill_core::is_low_content_markdown;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("distill-tui-test-{unique}-{name}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn reads_batch_files_and_skips_comments() {
        let dir = temp_dir("batch");
        let batch_path = dir.join("urls.txt");
        fs::write(
            &batch_path,
            "https://example.com/one\n# comment\n\nhttps://example.com/two\n",
        )
        .expect("write batch file");

        let urls = read_batch_file(&batch_path).expect("read batch file");

        assert_eq!(urls, vec!["https://example.com/one", "https://example.com/two"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn exports_combined_markdown_and_zip() {
        let dir = temp_dir("exports");
        fs::write(dir.join("first.md"), "# First\n").expect("write first markdown");
        fs::write(dir.join("second.md"), "# Second\n").expect("write second markdown");

        let combined = export_combined(&dir).expect("export combined markdown");
        let zip = export_zip(&dir).expect("export zip");
        let files = collect_markdown_files(&dir).expect("collect markdown files");

        let combined_contents = fs::read_to_string(&combined).expect("read combined markdown");

        assert_eq!(files.len(), 2);
        assert!(combined_contents.contains("first.md"));
        assert!(combined_contents.contains("# First"));
        assert!(combined_contents.contains("# Second"));
        assert!(zip.is_file());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_frontmatter_only_output_as_low_content() {
        let markdown = "---\ntitle: 'Example'\n---\n\n";
        assert!(is_low_content_markdown(markdown));
    }

    #[test]
    fn does_not_flag_real_markdown_content_as_low_content() {
        let markdown = r#"---
title: 'Example'
---

# Example

RustCor builds secure, high-performance Rust software for operators.

## Products

- Zenith
- RustMon
"#;

        assert!(!is_low_content_markdown(markdown));
    }
}
