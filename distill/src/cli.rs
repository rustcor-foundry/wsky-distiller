use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "distill",
    version = "1.0.0",
    about = "Fast, ethical webpage to clean Markdown for AI"
)]
pub struct Args {
    /// Single URL to distill
    #[arg(conflicts_with_all = ["batch", "stdin"])]
    pub url: Option<String>,

    /// File with URLs for batch processing
    #[arg(long, conflicts_with = "stdin")]
    pub batch: Option<String>,

    /// Output directory for batch mode
    #[arg(long, default_value = "distilled")]
    pub output_dir: String,

    /// Output file for single mode
    #[arg(short, long)]
    pub output: Option<String>,

    /// Read from stdin
    #[arg(long, conflicts_with = "batch")]
    pub stdin: bool,

    /// Enable fast conversion (requires --features fast)
    #[arg(long)]
    pub fast: bool,

    /// Include images in output
    #[arg(long)]
    pub include_images: bool,

    /// Skip frontmatter
    #[arg(long)]
    pub no_frontmatter: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value = "rich")]
    pub format: Format,

    /// Minimum delay between requests (ms)
    #[arg(long, default_value = "500")]
    pub delay: u64,

    /// Max parallel requests
    #[arg(long, default_value = "8")]
    pub concurrency: usize,

    /// Respect robots.txt
    #[arg(long, default_value = "true")]
    pub respect_robots: bool,

    /// Proxy file or single proxy
    #[arg(long)]
    pub proxies: Option<String>,

    #[arg(long)]
    pub proxy: Option<String>,

    /// Preview output without writing files
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum Format {
    Rich,
    Standard,
    Minimal,
}
