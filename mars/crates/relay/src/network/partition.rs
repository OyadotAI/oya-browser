/** Native storage routing shares cookie site classification without consulting page-supplied keys. */
use ::url::Url;

/** Schemeful sites isolate protocols and registrable domains while ignoring URL paths and ports. */
pub fn partition_site(url: &Url) -> Option<String> {
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?;
    Some(format!("{}://{}", url.scheme(), super::cookies::site(host)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Same-site top-level paths and subdomains must not fragment a storage partition. */
    #[test]
    fn partition_ignores_paths_ports_queries_and_subdomains() {
        let first = Url::parse("https://a.example.co.uk:8443/a?q=one").unwrap();
        let second = Url::parse("https://b.example.co.uk/b?q=two").unwrap();
        assert_eq!(partition_site(&first), partition_site(&second));
        assert_eq!(
            partition_site(&first).as_deref(),
            Some("https://example.co.uk")
        );
    }

    /** Private suffix tenants and different schemes remain separate storage principals. */
    #[test]
    fn partition_preserves_scheme_and_private_suffix_isolation() {
        let site = |value| partition_site(&Url::parse(value).unwrap());
        assert_ne!(site("https://a.github.io"), site("https://b.github.io"));
        assert_ne!(site("https://example.com"), site("http://example.com"));
        assert_ne!(site("https://example.com"), site("https://example.org"));
    }

    /** IP literals remain exact and opaque/non-network URLs never share a fabricated key. */
    #[test]
    fn partition_retains_ip_literals_and_rejects_opaque_urls() {
        let site = |value| partition_site(&Url::parse(value).unwrap());
        assert_eq!(
            site("https://[::1]:8443/a").as_deref(),
            Some("https://[::1]")
        );
        assert_eq!(
            site("http://127.0.0.1:8080").as_deref(),
            Some("http://127.0.0.1")
        );
        assert_eq!(site("data:text/plain,hello"), None);
        assert_eq!(site("file:///tmp/page.html"), None);
    }
}
