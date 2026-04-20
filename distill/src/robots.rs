use reqwest::blocking::Client;
use robotstxt::DefaultMatcher;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use url::Url;

pub struct RobotsChecker {
    robots_agent: String,
    client: Client,
    cache: Mutex<HashMap<String, String>>,
}

impl RobotsChecker {
    pub fn new(http_user_agent: &str, robots_agent: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(http_user_agent)
            .build()
            .expect("valid robots client");

        Self {
            robots_agent: robots_agent.to_string(),
            client,
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn is_allowed(&self, target_url: &str) -> bool {
        let parsed = match Url::parse(target_url) {
            Ok(url) => url,
            Err(_) => return true,
        };

        let cache_key = cache_key(&parsed);
        let robots_body = self.get_rules_for_origin(&parsed, &cache_key);
        let mut matcher = DefaultMatcher::default();
        matcher.one_agent_allowed_by_robots(&robots_body, &self.robots_agent, target_url)
    }

    fn get_rules_for_origin(&self, parsed: &Url, cache_key: &str) -> String {
        if let Ok(cache) = self.cache.lock() {
            if let Some(rules) = cache.get(cache_key) {
                return rules.clone();
            }
        }

        let rules = self.fetch_rules(parsed).unwrap_or_default();

        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(cache_key.to_string(), rules.clone());
        }

        rules
    }

    fn fetch_rules(&self, parsed: &Url) -> Option<String> {
        let mut robots_url = parsed.clone();
        robots_url.set_path("/robots.txt");
        robots_url.set_query(None);
        robots_url.set_fragment(None);

        let response = self.client.get(robots_url).send().ok()?;
        if !response.status().is_success() {
            return Some(String::new());
        }

        response.text().ok()
    }
}

fn cache_key(parsed: &Url) -> String {
    let mut key = format!(
        "{}://{}",
        parsed.scheme(),
        parsed.host_str().unwrap_or_default()
    );
    if let Some(port) = parsed.port() {
        key.push(':');
        key.push_str(&port.to_string());
    }
    key
}

#[cfg(test)]
mod tests {
    use robotstxt::DefaultMatcher;

    #[test]
    fn honors_longest_match_precedence() {
        let robots = "User-agent: distill\nDisallow: /private\nAllow: /private/public\n";
        let mut matcher = DefaultMatcher::default();

        assert!(matcher.one_agent_allowed_by_robots(
            robots,
            "distill",
            "https://example.com/private/public/page"
        ));
        assert!(!matcher.one_agent_allowed_by_robots(
            robots,
            "distill",
            "https://example.com/private/secret"
        ));
    }
}
