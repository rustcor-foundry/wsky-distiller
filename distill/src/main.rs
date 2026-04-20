use clap::Parser;
use distill_core::cli::Args;
use distill_core::{batch, fetch, markdown_from_html, robots, DistillOptions};
use std::io::Read;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.batch.is_some() {
        return batch::run_batch(&args);
    }

    run_single(&args)
}

fn run_single(args: &Args) -> anyhow::Result<()> {
    let input = resolve_single_input(args)?;

    let html = if args.stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        if buf.trim().is_empty() {
            anyhow::bail!("No input received on stdin");
        }
        buf
    } else if input.is_local_file {
        std::fs::read_to_string(&input.value)?
    } else {
        let proxy = batch::resolve_proxy(args)?;
        if args.respect_robots {
            let checker =
                robots::RobotsChecker::new("Mozilla/5.0 (compatible; distill/1.0)", "distill");
            if !checker.is_allowed(&input.value) {
                anyhow::bail!("Blocked by robots.txt: {}", input.value);
            }
        }
        fetch::fetch_with_retry(&input.value, proxy.as_deref(), 3)?
    };

    let source = if args.stdin || input.is_local_file {
        None
    } else {
        Some(input.value.as_str())
    };
    let md = markdown_from_html(
        &html,
        source,
        &DistillOptions {
            include_images: args.include_images,
            no_frontmatter: args.no_frontmatter,
            format: args.format.clone(),
            fast: args.fast,
        },
    )?;

    if args.dry_run {
        eprintln!("DRY RUN MODE - previewing output without writing files");
        println!("{}", md);
        return Ok(());
    }

    if let Some(path) = &args.output {
        std::fs::write(path, md)?;
        println!("Saved distilled output to {}", path);
    } else {
        println!("{}", md);
    }

    Ok(())
}

struct SingleInput {
    value: String,
    is_local_file: bool,
}

fn resolve_single_input(args: &Args) -> anyhow::Result<SingleInput> {
    if args.stdin {
        return Ok(SingleInput {
            value: "stdin".to_string(),
            is_local_file: false,
        });
    }

    let value = args
        .url
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Provide a URL, a local HTML file, or pass --stdin"))?;
    let is_local_file = Path::new(&value).is_file();

    Ok(SingleInput {
        value,
        is_local_file,
    })
}
