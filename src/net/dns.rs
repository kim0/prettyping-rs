use std::net::{IpAddr, ToSocketAddrs};

use thiserror::Error;

use crate::config::AddressFamily;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveResult {
    pub host: String,
    pub addresses: Vec<IpAddr>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ResolveError {
    #[error("host must not be empty")]
    EmptyHost,
    #[error("failed to resolve '{host}': {message}")]
    LookupFailed { host: String, message: String },
    #[error("resolved '{host}' but no address matches family {family:?}")]
    NoAddressForFamily { host: String, family: AddressFamily },
}

pub fn resolve_once(host: &str, family: AddressFamily) -> Result<ResolveResult, ResolveError> {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        return Err(ResolveError::EmptyHost);
    }

    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        if matches_family(ip, family) {
            return Ok(ResolveResult {
                host: trimmed.to_string(),
                addresses: vec![ip],
            });
        }
        return Err(ResolveError::NoAddressForFamily {
            host: trimmed.to_string(),
            family,
        });
    }

    let lookup = (trimmed, 0)
        .to_socket_addrs()
        .map_err(|err| ResolveError::LookupFailed {
            host: trimmed.to_string(),
            message: err.to_string(),
        })?;

    let mut addresses = Vec::new();
    for socket_addr in lookup {
        let ip = socket_addr.ip();
        if matches_family(ip, family) && !addresses.contains(&ip) {
            addresses.push(ip);
        }
    }

    if addresses.is_empty() {
        return Err(ResolveError::NoAddressForFamily {
            host: trimmed.to_string(),
            family,
        });
    }

    Ok(ResolveResult {
        host: trimmed.to_string(),
        addresses,
    })
}

fn matches_family(ip: IpAddr, family: AddressFamily) -> bool {
    match family {
        AddressFamily::Any => true,
        AddressFamily::Ipv4 => ip.is_ipv4(),
        AddressFamily::Ipv6 => ip.is_ipv6(),
    }
}

#[cfg(test)]
mod tests {
    use super::{AddressFamily, ResolveError, resolve_once};

    #[test]
    fn literal_ipv6_is_rejected_when_ipv4_is_requested() {
        let err = resolve_once("::1", AddressFamily::Ipv4).expect_err("must reject v6 for -4");
        assert!(matches!(err, ResolveError::NoAddressForFamily { .. }));
    }

    #[test]
    fn literal_ipv4_is_rejected_when_ipv6_is_requested() {
        let err =
            resolve_once("127.0.0.1", AddressFamily::Ipv6).expect_err("must reject v4 for -6");
        assert!(matches!(err, ResolveError::NoAddressForFamily { .. }));
    }
}
