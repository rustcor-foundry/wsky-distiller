use anyhow::anyhow;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use clap::Parser;
use distill_core::{markdown_from_html, DistillOptions, Format};
use futures::StreamExt;
use rand::Rng;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "distill-render",
    version,
    about = "Render JS-heavy pages with stealth"
)]
struct Args {
    /// URL to render
    url: String,

    /// Output Markdown file (stdout if not provided)
    #[arg(short, long)]
    output: Option<String>,

    /// Wait strategy before extracting HTML
    #[arg(long, value_enum, default_value_t = WaitStrategy::Networkidle)]
    wait: WaitStrategy,

    /// Stealth mode (recommended)
    #[arg(long, default_value = "true")]
    stealth: bool,

    /// Timeout in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Include images in output
    #[arg(long)]
    include_images: bool,

    /// Skip frontmatter
    #[arg(long)]
    no_frontmatter: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value = "rich")]
    format: Format,

    /// Enable fast conversion (requires distill to be built with --features fast)
    #[arg(long)]
    fast: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum WaitStrategy {
    Load,
    Domcontentloaded,
    Networkidle,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (width, height, user_agent) = stealth_profile(args.stealth);

    eprintln!("Launching browser...");

    let (mut browser, mut handler) = Browser::launch(
        BrowserConfig::builder()
            .window_size(width, height)
            .build()
            .map_err(|e| anyhow!(e))?,
    )
    .await?;

    tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if let Err(e) = h {
                eprintln!("Handler error: {:?}", e);
            }
        }
    });

    let page: Page = browser.new_page("about:blank").await?;

    tokio::time::timeout(
        Duration::from_secs(args.timeout),
        page.set_user_agent(user_agent.as_str()),
    )
    .await
    .map_err(|_| anyhow!("Timed out configuring browser page"))??;

    if args.stealth {
        apply_advanced_stealth(&page).await?;
    }

    tokio::time::timeout(
        Duration::from_secs(args.timeout),
        page.goto(args.url.as_str()),
    )
    .await
    .map_err(|_| anyhow!("Timed out opening page"))??;

    match args.wait {
        WaitStrategy::Networkidle => {
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }
        WaitStrategy::Domcontentloaded => {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        WaitStrategy::Load => {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    let html = tokio::time::timeout(Duration::from_secs(args.timeout), page.content())
        .await
        .map_err(|_| anyhow!("Timed out extracting rendered HTML"))??;

    let markdown = markdown_from_html(
        &html,
        Some(args.url.as_str()),
        &DistillOptions {
            include_images: args.include_images,
            no_frontmatter: args.no_frontmatter,
            format: args.format,
            fast: args.fast,
        },
    )?;

    if let Some(path) = args.output {
        std::fs::write(&path, &markdown)?;
        eprintln!("Saved distilled Markdown to {}", path);
    } else {
        println!("{}", markdown);
    }

    browser.close().await?;
    Ok(())
}

async fn apply_advanced_stealth(page: &Page) -> anyhow::Result<()> {
    // 1. Remove webdriver
    page.evaluate(
        r#"
        Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
    "#,
    )
    .await?;

    // 2. Spoof plugins and languages
    page.evaluate(
        r#"
        Object.defineProperty(navigator, 'plugins', { get: () => [1, 2, 3] });
        Object.defineProperty(navigator, 'languages', { get: () => ['en-US', 'en'] });
        Object.defineProperty(navigator, 'platform', { get: () => 'Win32' });
    "#,
    )
    .await?;

    // 3. WebGL / Canvas fingerprint spoofing
    page.evaluate(
        r#"
        const getParameter = WebGLRenderingContext.prototype.getParameter;
        WebGLRenderingContext.prototype.getParameter = function(parameter) {
            if (parameter === 37445) return 'Intel Inc.';
            if (parameter === 37446) return 'Intel(R) HD Graphics 630';
            return getParameter.call(this, parameter);
        };
    "#,
    )
    .await?;

    Ok(())
}

fn stealth_profile(stealth: bool) -> (u32, u32, String) {
    if !stealth {
        return (
            1920,
            1080,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".to_string(),
        );
    }

    let user_agents = [
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
    ];
    let mut rng = rand::thread_rng();
    let user_agent = user_agents[rng.gen_range(0..user_agents.len())].to_string();
    let width = rng.gen_range(1200..1920);
    let height = rng.gen_range(700..1080);

    (width, height, user_agent)
}
