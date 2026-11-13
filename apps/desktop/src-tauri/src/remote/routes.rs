use std::net::Ipv4Addr;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Lan,
    Tailscale,
    Public,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Route {
    pub kind: Kind,
    pub url: String,
}

fn host(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split('/').next()?;
    Some(
        authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host),
    )
}

fn classify(url: &str) -> Option<Kind> {
    if url.starts_with("https://") {
        return Some(Kind::Public);
    }
    let ip: Ipv4Addr = host(url)?.parse().ok()?;
    if ip.is_loopback() || ip.is_unspecified() {
        return None;
    }
    let [a, b, ..] = ip.octets();
    if a == 100 && (64..128).contains(&b) {
        Some(Kind::Tailscale)
    } else {
        Some(Kind::Lan)
    }
}

pub fn ordered(urls: &[String]) -> Vec<Route> {
    let mut routes: Vec<Route> = urls
        .iter()
        .filter_map(|url| {
            classify(url).map(|kind| Route {
                kind,
                url: url.clone(),
            })
        })
        .collect();
    routes.sort_by_key(|route| match route.kind {
        Kind::Lan => 0,
        Kind::Tailscale => 1,
        Kind::Public => 2,
    });
    routes
}
