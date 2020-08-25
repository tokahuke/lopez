//! Robot exclusion protocol (robots.txt) compliance.
//! TODO. Maybe use Aho-Corasick here is a good idea but we need to play smart.

use lazy_static::lazy_static;
use reqwest::Client;
use robots_txt::Robots;
use url::{Position, Url};

pub struct RobotExclusion {
    disallow: Vec<Match>,
    crawl_delay: Option<f64>,
}

impl RobotExclusion {
    pub fn new(robots_str: &str) -> RobotExclusion {
        let robots = Robots::from_str_lossy(robots_str);
        let my_section = robots.choose_section("lopez");
        RobotExclusion {
            disallow: my_section
                .rules
                .iter()
                .filter(|rule| !rule.allow)
                .map(|rule| Match::new(&rule.path))
                .collect::<Vec<_>>(),
            crawl_delay: my_section
                .crawl_delay
                .or(my_section.req_rate.and_then(|req_rate| {
                    if req_rate.requests > 0 {
                        Some(req_rate.seconds as f64 / req_rate.requests as f64)
                    } else {
                        None
                    }
                })),
        }
    }

    pub fn crawl_delay(&self) -> Option<f64> {
        self.crawl_delay
    }

    pub fn allows(&self, url: &Url) -> bool {
        !self
            .disallow
            .iter()
            .any(|match_rule| match_rule.matches(&url[Position::BeforePath..]))
    }
}

struct Match {
    match_str: String,
    is_strict: bool,
}

impl Match {
    fn new(path: &str) -> Match {
        if path.ends_with('$') {
            Match {
                match_str: path[..path.len() - 1].to_owned(),
                is_strict: true,
            }
        } else {
            Match {
                match_str: path.to_owned(),
                is_strict: false,
            }
        }
    }

    fn matches(&self, mut route: &str) -> bool {
        // random corner case I have found people use:
        if self.match_str.is_empty() {
            return false;
        }

        for pattern in self.match_str.split('*') {
            if let Some(found) = route.find(pattern) {
                route = &route[found..];
            } else {
                return false;
            }
        }

        // `is_strict` implies route must have been consumed at this point.
        !self.is_strict || route.is_empty() // "not a or b" the same as "if a then b"
    }
}

#[test]
fn robots_test() {
    let robots_txt = r#"
# See http://www.robotstxt.org/wc/norobots.html for documentation on how to use the robots.txt file
#
# To ban all spiders from the entire site uncomment the next two lines:
# User-Agent: *
# Disallow: /

# Ban Grapeshot
User-Agent: grapeshot
Disallow: /

# Ban oauth urls
User-Agent: *
Disallow: /auth/
Disallow: /busca-cursos/resultados
Disallow: /login
Disallow: /pre-matricula
Disallow: /revista/admin/
Disallow: /intercambio/estudar-no-exterior
Disallow: /intercambio?

# Ban api urls
Disallow: /api/

Sitemap: https://querobolsa.com.br/sitemap_index.xml
"#;

    let robots = Robots::from_str_lossy(robots_txt);
    // println!("{:#?}", robots);
    println!("{:#?}", robots.choose_section("lopez"));
}

lazy_static! {
    static ref CLIENT: Client = Client::builder()
        .pool_max_idle_per_host(0)
        .build()
        .expect("can always build robots fetching reqwest::Client");
}

pub async fn get_robots(base_url: &Url, user_agent: &str) -> Result<Option<String>, crate::Error> {
    // Make the request.
    let robots_url: Url = base_url.join("/robots.txt")?;
    let response = CLIENT
        .get(robots_url.clone())
        .header("User-Agent", user_agent)
        .send()
        .await?;
    let status_code = response.status();

    // Get status and filter failures:
    if status_code.is_success() {
        Ok(Some(response.text().await?))
    } else {
        log::warn!("robots route unsuccessful for `{}`", robots_url);
        Ok(None)
    }
}

#[tokio::test]
async fn test_get_robots() {
    let robots = get_robots(&"http://querobolsa.com.br".parse().unwrap(), "hello!")
        .await
        .unwrap()
        .unwrap();
    println!("{}", robots);
}
