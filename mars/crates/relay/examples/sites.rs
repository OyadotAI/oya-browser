/** Fetches each public site through Relay, following redirects with a cookie jar, and reports status, protocol and timing. */
use mars_relay::{CookieJar, Prepared, WebRequest, policy, redirect_url};
use std::time::{Instant, SystemTime};

/** Each site gets the whole navigation budget. */
const REMAINING_MS: u64 = 8_000;

/** Redirect hops followed per site, as a browser would before giving up. */
const REDIRECTS: usize = 10;

/** Usage: `sites <file>`; one URL per line, `#` lines ignored. Prints one tab-separated row per site. */
fn main() {
    let path = std::env::args().nth(1).expect("usage: sites <file>");
    let list = std::fs::read_to_string(path).expect("site list");
    let mut jar = CookieJar::default();
    println!("site\tstatus\tprotocol\tms\tbytes\tfinal");
    for site in list
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let start = Instant::now();
        let row = match visit(site, &mut jar) {
            Ok((status, protocol, bytes, last)) => {
                format!("{status}\t{protocol}\t{{ms}}\t{bytes}\t{last}")
            }
            Err(error) => format!("ERR\t-\t{{ms}}\t0\t{error}"),
        };
        let ms = start.elapsed().as_millis();
        println!("{site}\t{}", row.replace("{ms}", &ms.to_string()));
    }
}

/** One top-level navigation: GET, store cookies, follow Location until a non-redirect. */
fn visit(site: &str, jar: &mut CookieJar) -> Result<(u16, &'static str, usize, String), String> {
    let mut url = policy::web_url(site).map_err(|e| e.message)?;
    for _ in 0..=REDIRECTS {
        let now = SystemTime::now();
        let request = WebRequest {
            cookie: jar.header(&url, None, true, now),
            top_level: Some(url.to_string()),
            ..WebRequest::get(url.as_str())
        };
        let prepared = Prepared::connect(request, &[], REMAINING_MS).map_err(|e| e.message)?;
        let protocol = prepared.protocol();
        let response = prepared.transmit().map_err(|e| e.message)?;
        jar.receive(&url, &response.set_cookies, now);
        match response
            .location
            .as_deref()
            .filter(|_| (300..400).contains(&response.status))
        {
            Some(location) => url = redirect_url(&url, location, &[]).map_err(|e| e.message)?,
            None => {
                return Ok((
                    response.status,
                    protocol,
                    response.source.len(),
                    url.to_string(),
                ));
            }
        }
    }
    Err("redirect limit".into())
}
