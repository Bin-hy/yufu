use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Clone, Debug)]
pub struct RouteEntry {
    pub domain: String,
    pub digest: String,
}

pub fn build_route_entries(text: &str) -> Vec<RouteEntry> {
    let mut seen_links = HashSet::new();
    let mut seen_domains = HashSet::new();
    let mut routes = Vec::new();

    collect_routes(
        text,
        "https://",
        '/',
        &mut seen_links,
        &mut seen_domains,
        &mut routes,
    );
    collect_routes(
        text,
        "https:\\/\\/",
        '\\',
        &mut seen_links,
        &mut seen_domains,
        &mut routes,
    );

    routes
}

pub fn build_digest_map(routes: &[RouteEntry]) -> HashMap<String, String> {
    routes
        .iter()
        .map(|route| (route.digest.clone(), route.domain.clone()))
        .collect()
}

pub fn build_upstream_url(
    digest_map: &HashMap<String, String>,
    path: &str,
    query: Option<&str>,
) -> Option<String> {
    let trimmed = path.trim_start_matches('/');
    let (digest, rest) = trimmed
        .split_once('/')
        .map_or((trimmed, ""), |(a, b)| (a, b));
    let domain = digest_map.get(digest)?;

    let mut url = if rest.is_empty() {
        format!("https://{domain}/")
    } else {
        format!("https://{domain}/{rest}")
    };

    if let Some(query) = query {
        url.push('?');
        url.push_str(query);
    }

    Some(url)
}

pub fn fallback_upstream_url(path: &str, query: Option<&str>) -> Option<String> {
    let trimmed = path.trim_start_matches('/');
    let (_digest, rest) = trimmed.split_once('/')?;
    if rest.is_empty() {
        return None;
    }
    let mut url = format!("https://maps.googleapis.com/{rest}");
    if let Some(query) = query {
        url.push('?');
        url.push_str(query);
    }
    Some(url)
}

pub fn rewrite_target(base: &str, digest: &str) -> String {
    if base.is_empty() {
        format!("/{digest}")
    } else {
        format!("{}/{digest}", base.trim_end_matches('/'))
    }
}

pub fn rewrite_urls_with_base(input: &str, routes: &[RouteEntry], base: &str) -> String {
    let mut output = input.to_string();

    for route in routes {
        let target = rewrite_target(base, &route.digest);
        output = output.replace(&format!("https://{}", route.domain), &target);
        output = output.replace(
            &format!("https:\\/\\/{}", route.domain),
            &target.replace('/', "\\/"),
        );
    }

    output
}

fn collect_routes(
    text: &str,
    prefix: &str,
    path_separator: char,
    seen_links: &mut HashSet<String>,
    seen_domains: &mut HashSet<String>,
    routes: &mut Vec<RouteEntry>,
) {
    let mut index = 0;

    while let Some(found) = text[index..].find(prefix) {
        let start = index + found;
        let end = find_url_end(text, start, path_separator == '\\');
        let link = normalize_link(&text[start..end]);
        index = end;

        if !seen_links.insert(link.clone()) {
            continue;
        }

        let Some(domain) = link
            .strip_prefix(prefix)
            .and_then(|s| s.split(path_separator).next())
        else {
            continue;
        };

        if !seen_domains.insert(domain.to_string()) {
            continue;
        }

        routes.push(RouteEntry {
            domain: domain.to_string(),
            digest: short_hash(domain),
        });
    }
}

fn find_url_end(text: &str, start: usize, allow_backslash: bool) -> usize {
    let bytes = text.as_bytes();
    let mut end = start;
    while end < bytes.len() && !is_url_end(bytes[end], allow_backslash) {
        end += 1;
    }
    end
}

fn is_url_end(byte: u8, allow_backslash: bool) -> bool {
    matches!(
        byte,
        b' ' | b'\n' | b'\r' | b'\t' | b'"' | b'\'' | b'<' | b'>' | b')'
    ) || (!allow_backslash && byte == b'\\')
}

fn normalize_link(link: &str) -> String {
    link.trim_end_matches(&['"', '\'', ',', ';'][..])
        .to_string()
}

fn short_hash(domain: &str) -> String {
    let mut hasher = DefaultHasher::new();
    domain.replace('.', "").hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
