//! Local proxy validation and transport configuration for `Probe`.

use crate::{Error, Result};
use secrecy::{ExposeSecret, SecretString};
use std::process::Command;
use zeroize::Zeroizing;

struct ParsedProxy {
    protocol: ureq::ProxyProtocol,
    url: url::Url,
    port: u16,
    username: Option<Zeroizing<String>>,
    password: Option<Zeroizing<String>>,
}

fn parse_proxy(raw: &SecretString) -> Result<ParsedProxy> {
    let raw = raw.expose_secret();
    if raw.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(Error::InvalidProxy);
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(Error::InvalidProxy);
    }
    let url = url::Url::parse(trimmed).map_err(|_| Error::InvalidProxy)?;
    if url.cannot_be_a_base()
        || url.host_str().is_none_or(str::is_empty)
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::InvalidProxy);
    }
    let protocol = match url.scheme() {
        "http" => ureq::ProxyProtocol::Http,
        "https" => ureq::ProxyProtocol::Https,
        _ => return Err(Error::InvalidProxy),
    };
    let port = url
        .port_or_known_default()
        .filter(|port| *port != 0)
        .ok_or(Error::InvalidProxy)?;
    let (username, password) = if !url.username().is_empty() || url.password().is_some() {
        (
            Some(decode_percent(url.username())?),
            url.password().map(decode_percent).transpose()?,
        )
    } else {
        (None, None)
    };
    Ok(ParsedProxy {
        protocol,
        url,
        port,
        username,
        password,
    })
}

fn decode_percent(raw: &str) -> Result<Zeroizing<String>> {
    let bytes = raw.as_bytes();
    let mut decoded = Zeroizing::new(Vec::with_capacity(bytes.len()));
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hi = bytes.get(index + 1).copied().and_then(hex_value);
            let lo = bytes.get(index + 2).copied().and_then(hex_value);
            let (Some(hi), Some(lo)) = (hi, lo) else {
                return Err(Error::InvalidProxy);
            };
            decoded.push((hi << 4) | lo);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    if std::str::from_utf8(&decoded).is_err() {
        return Err(Error::InvalidProxy);
    }
    let text = String::from_utf8(std::mem::take(&mut *decoded)).map_err(|_| Error::InvalidProxy)?;
    Ok(Zeroizing::new(text))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(in crate::probe) fn env_no_proxy() -> Option<String> {
    ["NO_PROXY", "no_proxy"]
        .into_iter()
        .find_map(|variable| std::env::var(variable).ok())
}

pub(crate) fn build_ureq_proxy(raw: &SecretString, no_proxy: Option<&str>) -> Result<ureq::Proxy> {
    let parsed = parse_proxy(raw)?;
    let host = parsed.url.host_str().ok_or(Error::InvalidProxy)?;
    let mut builder = ureq::Proxy::builder(parsed.protocol)
        .host(host)
        .port(parsed.port);
    if let Some(username) = &parsed.username {
        builder = builder.username(username);
    }
    if let Some(password) = &parsed.password {
        builder = builder.password(password);
    }
    if let Some(no_proxy) = no_proxy {
        for entry in no_proxy
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            builder = builder.no_proxy(entry);
        }
    }
    builder.build().map_err(|_| Error::InvalidProxy)
}

pub(in crate::probe) fn command_proxy_value(raw: &SecretString) -> Result<Zeroizing<String>> {
    let parsed = parse_proxy(raw)?;
    Ok(Zeroizing::new(
        parsed.url.as_str().trim_end_matches('/').to_owned(),
    ))
}

pub(crate) fn local_command(program: &str, args: &[&str], proxy: Option<&str>) -> Command {
    let mut command = sheprd_client::new_command(program);
    command
        .args(args)
        .env("PATH", super::local_path())
        .env("NO_COLOR", "1");
    if let Some(proxy) = proxy {
        command.env_remove("ALL_PROXY");
        #[cfg(unix)]
        command.env_remove("all_proxy");
        command.env("HTTP_PROXY", proxy).env("HTTPS_PROXY", proxy);
        #[cfg(unix)]
        command.env("http_proxy", proxy).env("https_proxy", proxy);
    }
    command
}
