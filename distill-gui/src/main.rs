use eframe::egui;
use eframe::{Error as EframeError, Renderer};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use zip::write::FileOptions;

struct DistillApp {
    urls: Vec<String>,
    new_url: String,
    output_dir: String,
    use_render: bool,
    use_stealth: bool,
    delay_ms: u32,
    processing: bool,
    results: Vec<String>,
    status: String,
    worker: Option<WorkerHandle>,
}

struct WorkerHandle {
    rx: Receiver<WorkerEvent>,
    cancel: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

enum WorkerEvent {
    Status(String),
    Result(String),
    Finished,
}

#[derive(Clone)]
struct ProcessingConfig {
    urls: Vec<String>,
    output_dir: PathBuf,
    use_render: bool,
    use_stealth: bool,
    delay_ms: u32,
}

struct ToolPaths {
    distill: String,
    render: String,
}

impl Default for DistillApp {
    fn default() -> Self {
        Self {
            urls: vec![],
            new_url: String::new(),
            output_dir: default_output_dir().to_string_lossy().to_string(),
            use_render: false,
            use_stealth: true,
            delay_ms: 500,
            processing: false,
            results: vec![],
            status: "Idle".to_string(),
            worker: None,
        }
    }
}

impl Drop for DistillApp {
    fn drop(&mut self) {
        self.signal_stop();
    }
}

impl eframe::App for DistillApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker_events();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("distill-gui - Web to Markdown Workbench");

            ui.horizontal(|ui| {
                ui.label("URL:");
                ui.text_edit_singleline(&mut self.new_url);
                if ui.button("Add").clicked() && !self.new_url.trim().is_empty() {
                    self.urls.push(self.new_url.trim().to_string());
                    self.new_url.clear();
                }
            });

            ui.horizontal(|ui| {
                if ui.button("Upload URLs file").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        if let Ok(content) = fs::read_to_string(path) {
                            for line in content.lines() {
                                let trimmed = line.trim();
                                if !trimmed.is_empty() && !trimmed.starts_with('#') {
                                    self.urls.push(trimmed.to_string());
                                }
                            }
                        }
                    }
                }

                if ui.button("Clear Queue").clicked() && !self.processing {
                    self.urls.clear();
                    self.results.clear();
                }
            });

            ui.separator();

            ui.label("Queue:");
            let mut remove_index: Option<usize> = None;
            egui::ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui| {
                    for (i, url) in self.urls.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}. {}", i + 1, url));
                            if !self.processing && ui.button("x").clicked() {
                                remove_index = Some(i);
                            }
                        });
                    }
                });
            if let Some(i) = remove_index {
                self.urls.remove(i);
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.checkbox(&mut self.use_render, "Use distill-render first");
                ui.add_enabled_ui(self.use_render, |ui| {
                    ui.checkbox(&mut self.use_stealth, "Stealth mode");
                });
                ui.add(egui::Slider::new(&mut self.delay_ms, 0..=5000).text("Delay (ms)"));
            });

            ui.horizontal(|ui| {
                ui.label("Output folder:");
                ui.text_edit_singleline(&mut self.output_dir);
                if ui.button("Browse").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.output_dir = path.to_string_lossy().to_string();
                    }
                }
            });

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.processing && !self.urls.is_empty(),
                        egui::Button::new("Start"),
                    )
                    .clicked()
                {
                    self.start_worker();
                }

                if ui
                    .add_enabled(self.processing, egui::Button::new("Stop"))
                    .clicked()
                {
                    self.signal_stop();
                    self.status = "Stopping...".to_string();
                }
            });

            if self.processing {
                ui.spinner();
            }
            ui.label(format!("Status: {}", self.status));

            ui.separator();
            ui.label("Results:");
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .show(ui, |ui| {
                    for result in &self.results {
                        ui.label(result);
                    }
                });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Open Output Folder").clicked() {
                    match open::that(&self.output_dir) {
                        Ok(()) => self.status = format!("Opened {}", self.output_dir),
                        Err(e) => self.status = format!("Open folder failed: {}", e),
                    }
                }
                if ui.button("Export All as ZIP").clicked() {
                    match export_zip(Path::new(&self.output_dir)) {
                        Ok(path) => self.status = format!("ZIP created at {}", path.display()),
                        Err(e) => self.status = format!("ZIP export failed: {}", e),
                    }
                }
                if ui.button("Export as Single File").clicked() {
                    match export_combined(Path::new(&self.output_dir)) {
                        Ok(path) => self.status = format!("Combined file at {}", path.display()),
                        Err(e) => self.status = format!("Combine failed: {}", e),
                    }
                }
                if ui.button("Copy All to Clipboard").clicked() {
                    match collect_all_markdown(Path::new(&self.output_dir)) {
                        Ok(content) => {
                            ctx.output_mut(|o| o.copied_text = content);
                            self.status = "Copied markdown to clipboard".to_string();
                        }
                        Err(e) => self.status = format!("Copy failed: {}", e),
                    }
                }
            });
        });
    }
}

impl DistillApp {
    fn start_worker(&mut self) {
        let tools = match resolve_tool_paths() {
            Ok(t) => t,
            Err(e) => {
                self.status = e;
                return;
            }
        };

        let cfg = ProcessingConfig {
            urls: self.urls.clone(),
            output_dir: PathBuf::from(self.output_dir.clone()),
            use_render: self.use_render,
            use_stealth: self.use_stealth,
            delay_ms: self.delay_ms,
        };

        if let Err(e) = fs::create_dir_all(&cfg.output_dir) {
            self.status = format!("Cannot create output folder: {}", e);
            return;
        }

        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_worker = Arc::clone(&cancel);

        let join = thread::spawn(move || {
            for (i, url) in cfg.urls.iter().enumerate() {
                if cancel_worker.load(Ordering::Relaxed) {
                    let _ = tx.send(WorkerEvent::Status("Cancelled".to_string()));
                    break;
                }

                let _ = tx.send(WorkerEvent::Status(format!(
                    "Processing {}/{}",
                    i + 1,
                    cfg.urls.len()
                )));

                let out_file = cfg.output_dir.join(output_filename(url));

                let result = if cfg.use_render {
                    run_render_then_distill_cancellable(
                        &tools,
                        url,
                        &out_file,
                        cfg.use_stealth,
                        &cancel_worker,
                    )
                } else {
                    run_distill_direct_cancellable(&tools, url, &out_file, &cancel_worker)
                };

                match result {
                    Ok(()) => {
                        let _ = tx.send(WorkerEvent::Result(format!(
                            "OK: {} -> {}",
                            url,
                            out_file.display()
                        )));
                    }
                    Err(e) => {
                        let _ = tx.send(WorkerEvent::Result(format!("FAIL: {} - {}", url, e)));
                    }
                }

                if cfg.delay_ms > 0 {
                    thread::sleep(std::time::Duration::from_millis(cfg.delay_ms as u64));
                }
            }

            let _ = tx.send(WorkerEvent::Finished);
        });

        self.worker = Some(WorkerHandle {
            rx,
            cancel,
            join: Some(join),
        });
        self.processing = true;
        self.results.clear();
        self.status = "Worker started".to_string();
    }

    fn signal_stop(&mut self) {
        if let Some(worker) = &mut self.worker {
            worker.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn poll_worker_events(&mut self) {
        let mut mark_finished = false;

        if let Some(worker) = &self.worker {
            loop {
                match worker.rx.try_recv() {
                    Ok(WorkerEvent::Status(s)) => self.status = s,
                    Ok(WorkerEvent::Result(r)) => self.results.push(r),
                    Ok(WorkerEvent::Finished) => {
                        mark_finished = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        mark_finished = true;
                        break;
                    }
                }
            }
        }

        if mark_finished {
            if let Some(worker) = &mut self.worker {
                if let Some(join) = worker.join.take() {
                    let _ = join.join();
                }
            }
            self.worker = None;
            self.processing = false;
            self.status = "Done".to_string();
        }
    }
}

fn run_distill_direct_cancellable(
    tools: &ToolPaths,
    url: &str,
    output_file: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<(), String> {
    let mut command = Command::new(&tools.distill);
    command.arg(url).arg("-o").arg(output_file);

    let out = cancellable_output(&mut command, Some(cancel))?;

    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn run_render_then_distill_cancellable(
    tools: &ToolPaths,
    url: &str,
    output_file: &Path,
    stealth: bool,
    cancel: &Arc<AtomicBool>,
) -> Result<(), String> {
    let mut render_cmd = Command::new(&tools.render);
    render_cmd.arg(url).arg("-o").arg(output_file);
    if !stealth {
        render_cmd.arg("--stealth").arg("false");
    }

    let render_out = cancellable_output(&mut render_cmd, Some(cancel))?;

    if !render_out.status.success() {
        return Err(String::from_utf8_lossy(&render_out.stderr)
            .trim()
            .to_string());
    }
    Ok(())
}

fn cancellable_output(
    command: &mut Command,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<std::process::Output, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start process: {}", e))?;

    wait_for_child(&mut child, cancel)
}

fn wait_for_child(
    child: &mut Child,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<std::process::Output, String> {
    use std::io::Read;

    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            let _ = child.kill();
            return Err("cancelled".to_string());
        }

        match child.try_wait().map_err(|e| format!("wait failed: {}", e))? {
            Some(status) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();

                if let Some(mut pipe) = child.stdout.take() {
                    pipe.read_to_end(&mut stdout)
                        .map_err(|e| format!("failed reading stdout: {}", e))?;
                }
                if let Some(mut pipe) = child.stderr.take() {
                    pipe.read_to_end(&mut stderr)
                        .map_err(|e| format!("failed reading stderr: {}", e))?;
                }

                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            None => thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn resolve_tool_paths() -> Result<ToolPaths, String> {
    let exe = std::env::consts::EXE_SUFFIX;
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    let gui_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = gui_root.parent().map(Path::to_path_buf);

    let mut distill_candidates = Vec::new();
    let mut render_candidates = Vec::new();

    if let Some(dir) = &exe_dir {
        distill_candidates.push(dir.join(format!("distill{}", exe)));
        render_candidates.push(dir.join(format!("distill-render{}", exe)));
    }

    if let Some(root) = &workspace_root {
        distill_candidates.push(
            root.join("distill")
                .join("target")
                .join("release")
                .join(format!("distill{}", exe)),
        );
        distill_candidates.push(
            root.join("distill")
                .join("target")
                .join("debug")
                .join(format!("distill{}", exe)),
        );
        render_candidates.push(
            root.join("distill-render")
                .join("target")
                .join("release")
                .join(format!("distill-render{}", exe)),
        );
        render_candidates.push(
            root.join("distill-render")
                .join("target")
                .join("debug")
                .join(format!("distill-render{}", exe)),
        );
    }

    let distill = first_existing(&distill_candidates)
        .unwrap_or_else(|| PathBuf::from(format!("distill{}", exe)));
    let render = first_existing(&render_candidates)
        .unwrap_or_else(|| PathBuf::from(format!("distill-render{}", exe)));

    Ok(ToolPaths {
        distill: distill.to_string_lossy().to_string(),
        render: render.to_string_lossy().to_string(),
    })
}

fn default_output_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.join("distill-output");
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("distill-output")
}

fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| p.exists()).cloned()
}

fn output_filename(url: &str) -> String {
    let slug = url
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        "output.md".to_string()
    } else {
        format!("{}-{}.md", slug, short_hash(url))
    }
}

fn collect_markdown_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("cannot read output folder: {}", e))?
        .filter_map(|e| e.ok().map(|v| v.path()))
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
        .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some("combined.md"))
        .collect();

    files.sort();
    Ok(files)
}

fn collect_all_markdown(dir: &Path) -> Result<String, String> {
    let files = collect_markdown_files(dir)?;
    if files.is_empty() {
        return Err("no markdown files found".to_string());
    }

    let mut out = String::new();
    for path in files {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("failed reading {}: {}", path.display(), e))?;
        out.push_str(&format!("\n\n<!-- {} -->\n\n", path.display()));
        out.push_str(&content);
    }
    Ok(out)
}

fn export_combined(dir: &Path) -> Result<PathBuf, String> {
    let combined = collect_all_markdown(dir)?;
    let target = dir.join("combined.md");
    fs::write(&target, combined).map_err(|e| format!("failed writing combined file: {}", e))?;
    Ok(target)
}

fn export_zip(dir: &Path) -> Result<PathBuf, String> {
    let files = collect_markdown_files(dir)?;
    if files.is_empty() {
        return Err("no markdown files found".to_string());
    }

    let target = dir.join("distilled.zip");
    let file = fs::File::create(&target).map_err(|e| format!("failed creating zip: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for path in files {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "invalid UTF-8 filename".to_string())?;
        zip.start_file(name, options)
            .map_err(|e| format!("zip start_file failed: {}", e))?;
        let content =
            fs::read(&path).map_err(|e| format!("failed reading {}: {}", path.display(), e))?;
        zip.write_all(&content)
            .map_err(|e| format!("zip write failed: {}", e))?;
    }

    zip.finish()
        .map_err(|e| format!("zip finish failed: {}", e))?;
    Ok(target)
}

fn short_hash(input: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())[..8].to_string()
}

fn main() -> eframe::Result<()> {
    match run_app(Renderer::Wgpu) {
        Ok(()) => Ok(()),
        Err(err) if should_fallback_to_glow(&err) => {
            eprintln!("WGPU unavailable, falling back to Glow renderer");
            run_app(Renderer::Glow)
        }
        Err(err) => Err(err),
    }
}

fn run_app(renderer: Renderer) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        renderer,
        ..Default::default()
    };
    eframe::run_native(
        "distill-gui",
        options,
        Box::new(|_cc| Box::new(DistillApp::default())),
    )
}

fn should_fallback_to_glow(err: &EframeError) -> bool {
    matches!(err, EframeError::Wgpu(inner) if inner.to_string().contains("no suitable adapter found"))
}
