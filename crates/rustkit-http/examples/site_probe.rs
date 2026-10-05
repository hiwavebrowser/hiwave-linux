//! Z2-T1 harness probe: one GET / per host with the shipped client plus the loader's Accept-Language, one JSON line each.
//! `cargo run -p rustkit-http --example site_probe -- www.ebay.com www.cars.com ...`
use rustkit_http::Client;

fn title(body: &str) -> String {
    let lower = body.to_ascii_lowercase();
    let Some(s) = lower.find("<title") else { return String::new() };
    let Some(gt) = lower[s..].find('>') else { return String::new() };
    let start = s + gt + 1;
    let Some(end) = lower[start..].find("</title>") else { return String::new() };
    body[start..start + end].split_whitespace().collect::<Vec<_>>().join(" ").chars().take(80).collect()
}

fn is_pass(status: u16, body: &str, title: &str, server_challenge: bool) -> bool {
    const BAD: [&str; 11] = [
        "just a moment", "access denied", "pardon our interruption", "robot or human", "captcha",
        "attention required", "are you a human", "verify you are", "denied", "blocked", "security check",
    ];
    let t = title.to_ascii_lowercase();
    let head = body.chars().take(6000).collect::<String>().to_ascii_lowercase();
    let chal = server_challenge
        || head.contains("challenge-platform")
        || head.contains("captcha-delivery.com")
        || head.contains("/_sec/cp_challenge");
    status == 200 && !chal && !BAD.iter().any(|b| t.contains(b)) && body.len() > 5000
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut hosts: Vec<String> = std::env::args().skip(1).collect();
    let chrome = hosts.iter().any(|a| a == "--chrome-handshake");
    hosts.retain(|a| a != "--chrome-handshake");
    let client = Client::new().expect("client");
    let client = if chrome {
        #[cfg(feature = "impersonate-test")]
        {
            client.with_test_chrome_handshake().expect("chrome handshake profile")
        }
        #[cfg(not(feature = "impersonate-test"))]
        {
            eprintln!("--chrome-handshake needs --features impersonate-test");
            std::process::exit(2);
        }
    } else {
        client
    };
    for host in hosts {
        let url = format!("https://{host}/");
        let mut headers = http::HeaderMap::new();
        headers.insert("accept-language", http::HeaderValue::from_static("en-US,en;q=0.9"));
        match client.request(http::Method::GET, &url, headers, None).await {
            Ok(r) => {
                let body = String::from_utf8_lossy(&r.body).into_owned();
                let t = title(&body);
                let mitigated = r.headers.contains_key("cf-mitigated");
                let pass = is_pass(r.status.as_u16(), &body, &t, mitigated);
                println!(
                    "{{\"host\":\"{host}\",\"verdict\":\"{}\",\"status\":{},\"version\":\"{:?}\",\"bytes\":{},\"title\":{:?}}}",
                    if pass { "PASS" } else { "BLOCK" }, r.status.as_u16(), r.version, r.body.len(), t
                );
            }
            Err(e) => println!("{{\"host\":\"{host}\",\"verdict\":\"ERR\",\"detail\":{:?}}}", e.to_string()),
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}
