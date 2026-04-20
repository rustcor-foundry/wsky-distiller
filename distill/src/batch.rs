use crate::cli::Args;
use crate::convert::convert_to_markdown;
use crate::extract::extract_content;
use crate::fetch::fetch_with_retry;
use crate::proxy::ProxyRotator;
use crate::robots::RobotsChecker;
use crate::utils::{apply_output_format, build_frontmatter, generate_filename};
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use rayon::ThreadPoolBuilder;
use std::fs;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub fn run_batch(args: &Args) -> Result<()> {
    let urls: Vec<String> = if let Some(batch_file) = &args.batch {
        BufReader::new(fs::File::open(batch_file)?)
            .lines()
            .filter_map(|l| l.ok())
            .filter(|l| {
                let trimmed = l.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .collect()
    } else {
        vec![]
    };

    if urls.is_empty() {
        anyhow::bail!("No URLs found");
    }

    if args.dry_run {
        println!("🔍 DRY RUN MODE - No files will be written");
        for url in &urls {
            println!("  Would process: {}", url);
        }
        return Ok(());
    }

    fs::create_dir_all(&args.output_dir)?;

    let proxies = load_proxies(args)?;
    let proxy_rotator = if proxies.is_empty() {
        None
    } else {
        Some(Arc::new(ProxyRotator::new(proxies)))
    };
    let robots = Arc::new(RobotsChecker::new(
        "Mozilla/5.0 (compatible; distill/1.0)",
        "distill",
    ));
    let throttle = Arc::new(RequestThrottle::new(args.delay));

    let pb = ProgressBar::new(urls.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap(),
    );
    let pb = Arc::new(pb);

    let success = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    let pool = ThreadPoolBuilder::new()
        .num_threads(args.concurrency.max(1))
        .build()?;

    pool.install(|| {
        urls.par_iter().for_each(|url| {
            throttle.wait();

            if args.respect_robots && !robots.is_allowed(url) {
                failed.fetch_add(1, Ordering::Relaxed);
                pb.println(format!("⛔ Skipped by robots.txt: {}", url));
                pb.inc(1);
                return;
            }

            let proxy = proxy_rotator.as_ref().and_then(|r| r.next());

            match process_url(url, args, proxy.as_deref()) {
                Ok(filename) => {
                    success.fetch_add(1, Ordering::Relaxed);
                    pb.println(format!("✅ {}", filename));
                }
                Err(e) => {
                    failed.fetch_add(1, Ordering::Relaxed);
                    pb.println(format!("❌ {} — {}", url, e));
                }
            }
            pb.inc(1);
        });
    });

    pb.finish_with_message("Complete!");

    println!(
        "\n📊 Summary: {} successful, {} failed",
        success.load(Ordering::Relaxed),
        failed.load(Ordering::Relaxed)
    );

    Ok(())
}

fn process_url(url: &str, args: &Args, proxy: Option<&str>) -> Result<String> {
    let html = fetch_with_retry(url, proxy, 3)?;
    let article = extract_content(&html, Some(url))?;
    let mut md = convert_to_markdown(&article.content, args.include_images, args.fast)?;
    md = apply_output_format(md, &args.format);

    if !args.no_frontmatter {
        let frontmatter = build_frontmatter(&article, Some(url));
        md = format!("{}{}", frontmatter, md);
    }

    let filename = generate_filename(url, &article.title);
    let path = std::path::Path::new(&args.output_dir).join(&filename);
    fs::write(&path, md)?;

    Ok(filename)
}

pub fn resolve_proxy(args: &Args) -> Result<Option<String>> {
    if let Some(proxy) = &args.proxy {
        return Ok(Some(proxy.clone()));
    }

    let proxies = load_proxies(args)?;
    Ok(proxies.into_iter().next())
}

fn load_proxies(args: &Args) -> Result<Vec<String>> {
    if let Some(path) = &args.proxies {
        let content = fs::read_to_string(path)?;
        let proxies = content
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(ToOwned::to_owned)
            .collect();
        return Ok(proxies);
    }

    Ok(vec![])
}

struct RequestThrottle {
    delay: Duration,
    next_allowed_at: Mutex<Option<Instant>>,
}

impl RequestThrottle {
    fn new(delay_ms: u64) -> Self {
        Self {
            delay: Duration::from_millis(delay_ms),
            next_allowed_at: Mutex::new(None),
        }
    }

    fn wait(&self) {
        if self.delay.is_zero() {
            return;
        }

        let mut next_allowed_at = self.next_allowed_at.lock().expect("request throttle mutex");
        let now = Instant::now();

        if let Some(deadline) = *next_allowed_at {
            if deadline > now {
                std::thread::sleep(deadline.duration_since(now));
            }
        }

        *next_allowed_at = Some(Instant::now() + self.delay);
    }
}

#[cfg(test)]
mod tests {
    use super::RequestThrottle;
    use std::time::{Duration, Instant};

    #[test]
    fn throttle_enforces_minimum_gap() {
        let throttle = RequestThrottle::new(25);
        let start = Instant::now();

        throttle.wait();
        throttle.wait();

        assert!(start.elapsed() >= Duration::from_millis(25));
    }
}
