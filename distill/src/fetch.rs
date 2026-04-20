use anyhow::Result;
use reqwest::blocking::Client;
use std::time::Duration;

pub fn fetch_with_retry(url: &str, proxy: Option<&str>, max_retries: u32) -> Result<String> {
    let mut last_error = None;

    for attempt in 0..=max_retries {
        match try_fetch(url, proxy) {
            Ok(html) => return Ok(html),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_retries {
                    let delay = 500 * 2u64.pow(attempt);
                    std::thread::sleep(Duration::from_millis(delay));
                }
            }
        }
    }

    Err(last_error.unwrap().context("All retry attempts failed"))
}

fn try_fetch(url: &str, proxy: Option<&str>) -> Result<String> {
    let mut builder = Client::builder()
        .user_agent("Mozilla/5.0 (compatible; distill/1.0)")
        .timeout(Duration::from_secs(30));

    if let Some(p) = proxy {
        builder = builder.proxy(reqwest::Proxy::all(p)?);
    }

    let client = builder.build()?;
    let response = client.get(url).send()?.error_for_status()?;
    Ok(response.text()?)
}
