use super::*;
use crate::{
    probe::{
        Request, http_local, local_agent_config,
        proxy::{build_ureq_proxy, local_command},
    },
    settings::ProviderSettings,
};
use secrecy::SecretString;
use std::{
    io::{ErrorKind, Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
};

struct ProxyCapture {
    connect_head: String,
    inner_head: String,
}

fn spawn_fake_http_proxy(response_body: &'static str) -> (u16, mpsc::Receiver<ProxyCapture>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let connect_head = read_http_head(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .unwrap();
        stream.flush().unwrap();
        let inner_head = read_http_head(&mut stream);
        let reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(reply.as_bytes()).unwrap();
        stream.flush().unwrap();
        let _ = tx.send(ProxyCapture {
            connect_head,
            inner_head,
        });
    });
    (port, rx)
}

fn spawn_fake_origin(response_body: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = read_http_head(&mut stream);
        let reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(reply.as_bytes()).unwrap();
        stream.flush().unwrap();
    });
    port
}

fn read_http_head(stream: &mut impl Read) -> String {
    let mut buf = Vec::new();
    let mut byte = [0_u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        let read = stream.read(&mut byte).unwrap();
        assert!(read > 0, "unexpected EOF before end of HTTP headers");
        buf.push(byte[0]);
    }
    String::from_utf8(buf).unwrap()
}

#[test]
fn configured_local_http_request_uses_provider_proxy() {
    let (port, rx) = spawn_fake_http_proxy(r#"{"ok":true}"#);
    let settings = ProviderSettings::default().with(
        "proxy",
        &format!("http://user%40ex:p%3Ass@127.0.0.1:{port}"),
    );
    let mut exec = Exec::Local;
    let mut jar = CookieJar::default();
    let mut probe = Probe::new(
        &mut exec,
        provider("codex"),
        Some(&settings),
        &mut jar,
        Consent::Quiet,
    );

    let response = probe
        .http(Request::get("http://nonexistent.invalid/usage"))
        .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#"{"ok":true}"#);

    let capture = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        capture
            .connect_head
            .starts_with("CONNECT nonexistent.invalid:80 HTTP/1.1\r\n"),
        "unexpected CONNECT request:\n{}",
        capture.connect_head
    );
    // Basic base64("user@ex:p:ss") == "dXNlckBleDpwOnNz"
    assert!(
        capture
            .connect_head
            .lines()
            .any(|line| line.eq_ignore_ascii_case("Proxy-Authorization: Basic dXNlckBleDpwOnNz")),
        "missing decoded Proxy-Authorization header:\n{}",
        capture.connect_head
    );
    assert!(
        capture.inner_head.starts_with("GET /usage HTTP/1.1\r\n"),
        "unexpected tunneled request:\n{}",
        capture.inner_head
    );
    assert!(
        capture
            .inner_head
            .lines()
            .any(|line| line.eq_ignore_ascii_case("Host: nonexistent.invalid")),
        "missing target Host header:\n{}",
        capture.inner_head
    );
}

#[test]
fn configured_proxy_overrides_local_command_env() {
    let parent_http = std::env::var_os("HTTP_PROXY");
    let parent_https = std::env::var_os("HTTPS_PROXY");
    let parent_no_proxy = std::env::var_os("NO_PROXY");

    let proxy = "http://127.0.0.1:17890";
    let command = local_command("agy", &["--version"], Some(proxy));
    let envs: Vec<_> = command
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|val| val.to_string_lossy().into_owned()),
            )
        })
        .collect();
    assert!(
        envs.iter()
            .any(|(k, v)| k == "HTTP_PROXY" && v.as_deref() == Some(proxy))
    );
    assert!(
        envs.iter()
            .any(|(k, v)| k == "HTTPS_PROXY" && v.as_deref() == Some(proxy))
    );
    assert!(!envs.iter().any(|(k, _)| k.eq_ignore_ascii_case("NO_PROXY")));

    let settings = ProviderSettings::default().with("proxy", proxy);
    let mut exec = Exec::Local;
    let mut jar = CookieJar::default();
    let mut probe = Probe::new(
        &mut exec,
        provider("antigravity"),
        Some(&settings),
        &mut jar,
        Consent::Quiet,
    );

    #[cfg(windows)]
    let output = probe
        .command(
            "cmd.exe",
            &["/D", "/C", "echo %HTTP_PROXY%^|%HTTPS_PROXY%^|%NO_PROXY%"],
            Duration::from_secs(5),
        )
        .unwrap();
    #[cfg(unix)]
    let output = probe
        .command(
            "/bin/sh",
            &[
                "-c",
                "printf '%s|%s|%s\n' \"$HTTP_PROXY\" \"$HTTPS_PROXY\" \"$NO_PROXY\"",
            ],
            Duration::from_secs(5),
        )
        .unwrap();

    assert!(output.success);
    let expected_no_proxy = parent_no_proxy
        .as_deref()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "%NO_PROXY%".to_owned()
            } else {
                String::new()
            }
        });
    assert_eq!(
        output.stdout.trim(),
        format!("{proxy}|{proxy}|{expected_no_proxy}")
    );
    assert_eq!(std::env::var_os("HTTP_PROXY"), parent_http);
    assert_eq!(std::env::var_os("HTTPS_PROXY"), parent_https);
    assert_eq!(std::env::var_os("NO_PROXY"), parent_no_proxy);
}

#[test]
fn configured_no_proxy_bypasses_explicit_proxy() {
    let origin_port = spawn_fake_origin("direct-origin");
    let decoy = TcpListener::bind("127.0.0.1:0").unwrap();
    decoy.set_nonblocking(true).unwrap();
    let decoy_port = decoy.local_addr().unwrap().port();

    let proxy = build_ureq_proxy(
        &SecretString::from(format!("http://127.0.0.1:{decoy_port}")),
        Some("localhost, 127.0.0.1, *.internal.test"),
    )
    .unwrap();
    assert!(proxy.is_no_proxy(&"http://127.0.0.1/usage".parse().unwrap()));
    assert!(proxy.is_no_proxy(&"https://api.internal.test/usage".parse().unwrap()));
    assert!(
        !proxy.is_no_proxy(
            &"https://chatgpt.com/backend-api/wham/usage"
                .parse()
                .unwrap()
        )
    );

    let response = http_local(
        &Request::get(format!("http://127.0.0.1:{origin_port}/usage")),
        Some(proxy),
    )
    .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, "direct-origin");
    assert!(matches!(decoy.accept(), Err(err) if err.kind() == ErrorKind::WouldBlock));
}

#[test]
fn unset_provider_proxy_keeps_environment_fallback() {
    let timeout = Duration::from_secs(5);
    assert_eq!(
        local_agent_config(timeout, None).proxy(),
        ureq::Agent::config_builder().build().proxy()
    );

    let empty = ProviderSettings::default().with("proxy", "   ");
    for settings in [None, Some(&ProviderSettings::default()), Some(&empty)] {
        let mut exec = Exec::Local;
        let mut jar = CookieJar::default();
        let probe = Probe::new(
            &mut exec,
            provider("codex"),
            settings,
            &mut jar,
            Consent::Quiet,
        );
        assert!(probe.local_proxy().is_none());
    }

    let command = local_command("agy", &["--version"], None);
    let envs: Vec<_> = command
        .get_envs()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();
    assert!(!envs.iter().any(|k| {
        k.eq_ignore_ascii_case("HTTP_PROXY")
            || k.eq_ignore_ascii_case("HTTPS_PROXY")
            || k.eq_ignore_ascii_case("ALL_PROXY")
            || k.eq_ignore_ascii_case("NO_PROXY")
    }));
}

#[test]
fn invalid_proxy_has_redacted_error() {
    let invalid_cases = [
        "socks5://sentinel-user:sentinel-secret@127.0.0.1:7890",
        "socks4://sentinel-user:sentinel-secret@127.0.0.1:7890",
        "ftp://sentinel-user:sentinel-secret@127.0.0.1:7890",
        "127.0.0.1:7890",
        "http://sentinel-user:sentinel-secret@127.0.0.1:7890/path",
        "http://sentinel-user:sentinel-secret@127.0.0.1:7890?query=1",
        "http://sentinel-user:sentinel-secret@127.0.0.1:7890#frag",
        "http://sentinel-user:sentinel-secret@:7890",
        "http://sentinel-user:sentinel-secret@127.0.0.1:0",
        "http://sentinel-user:bad%zz@127.0.0.1:17890",
        "http://sentinel-user:%FF@127.0.0.1:17890",
        "http://sentinel-user:sentinel-secret@127.0.0.1:17890\n",
        "http://sentinel-user:sentinel-secret@127.0.0.1:17890\nEVIL=1",
        "\n",
        "not a url sentinel-secret",
    ];
    for raw in invalid_cases {
        let settings = ProviderSettings::default().with("proxy", raw);
        let mut exec = Exec::Local;
        let mut jar = CookieJar::default();
        let mut probe = Probe::new(
            &mut exec,
            provider("codex"),
            Some(&settings),
            &mut jar,
            Consent::Quiet,
        );

        let http_err = probe
            .http(Request::get("http://nonexistent.invalid/usage"))
            .unwrap_err();
        assert!(
            matches!(http_err, Error::InvalidProxy),
            "{raw}: {http_err:?}"
        );
        assert!(!http_err.to_string().contains("sentinel"), "{http_err}");
        assert!(
            !format!("{http_err:?}").contains("sentinel"),
            "{http_err:?}"
        );

        let cmd_err = probe
            .command("agy", &["--version"], Duration::from_secs(1))
            .unwrap_err();
        assert!(matches!(cmd_err, Error::InvalidProxy), "{raw}: {cmd_err:?}");
        assert!(!cmd_err.to_string().contains("sentinel"), "{cmd_err}");
        assert!(!format!("{cmd_err:?}").contains("sentinel"), "{cmd_err:?}");
    }

    // Valid forms: https scheme, default port, trailing slash, IPv6, percent-encoded credentials.
    for valid in [
        "https://proxy.example",
        "http://127.0.0.1/",
        "http://[::1]:7890",
        "https://u%40ser:p%2540ss+1@proxy.example:8443",
    ] {
        let proxy = build_ureq_proxy(&SecretString::from(valid), None).unwrap();
        assert!(
            matches!(
                proxy.protocol(),
                ureq::ProxyProtocol::Http | ureq::ProxyProtocol::Https
            ),
            "{valid}"
        );
    }
}

#[test]
fn configured_proxy_is_provider_scoped() {
    let (codex_port, codex_rx) = spawn_fake_http_proxy(r#"{"provider":"codex"}"#);
    let (agy_port, agy_rx) = spawn_fake_http_proxy(r#"{"provider":"agy"}"#);

    let codex_settings =
        ProviderSettings::default().with("proxy", &format!("http://127.0.0.1:{codex_port}"));
    let agy_settings =
        ProviderSettings::default().with("proxy", &format!("http://127.0.0.1:{agy_port}"));

    let mut codex_exec = Exec::Local;
    let mut codex_jar = CookieJar::default();
    let mut codex_probe = Probe::new(
        &mut codex_exec,
        provider("codex"),
        Some(&codex_settings),
        &mut codex_jar,
        Consent::Quiet,
    );
    let response = codex_probe
        .http(Request::get("http://codex.invalid/usage"))
        .unwrap();
    assert_eq!(response.body, r#"{"provider":"codex"}"#);
    assert!(codex_rx.recv_timeout(Duration::from_secs(2)).is_ok());
    assert!(agy_rx.try_recv().is_err());

    let mut agy_exec = Exec::Local;
    let mut agy_jar = CookieJar::default();
    let mut agy_probe = Probe::new(
        &mut agy_exec,
        provider("antigravity"),
        Some(&agy_settings),
        &mut agy_jar,
        Consent::Quiet,
    );
    #[cfg(windows)]
    let output = agy_probe
        .command(
            "cmd.exe",
            &["/D", "/C", "echo %HTTP_PROXY%^|%HTTPS_PROXY%"],
            Duration::from_secs(5),
        )
        .unwrap();
    #[cfg(unix)]
    let output = agy_probe
        .command(
            "/bin/sh",
            &["-c", "printf '%s|%s\n' \"$HTTP_PROXY\" \"$HTTPS_PROXY\""],
            Duration::from_secs(5),
        )
        .unwrap();
    assert!(output.success);
    assert_eq!(
        output.stdout.trim(),
        format!("http://127.0.0.1:{agy_port}|http://127.0.0.1:{agy_port}")
    );
    assert!(agy_rx.try_recv().is_err());

    let mut claude_exec = Exec::Local;
    let mut claude_jar = CookieJar::default();
    let claude_probe = Probe::new(
        &mut claude_exec,
        provider("claude"),
        None,
        &mut claude_jar,
        Consent::Quiet,
    );
    assert!(claude_probe.local_proxy().is_none());
}

#[cfg(unix)]
#[test]
fn remote_probe_does_not_receive_local_proxy() {
    use crate::probe::Shell;

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let bin = home.join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = root.path().join("curl.log");
    crate::test_executable::write(
        bin.join("curl"),
        format!(
            "#!/bin/sh\nprintf 'env:%s|%s|%s|%s\\n' \"$HTTP_PROXY\" \"$HTTPS_PROXY\" \"$http_proxy\" \"$https_proxy\" >> '{log}'\nprintf '{{\"ok\":true}}\\n@@herdr-status 200'\n",
            log = log.display()
        ),
        0o755,
    )
    .unwrap();

    let mut command = std::process::Command::new("/bin/sh");
    command
        .arg("-s")
        .env_clear()
        .env("HOME", &home)
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let mut exec = Exec::Remote(Shell::start(command).unwrap());
    let settings = ProviderSettings::default().with("proxy", "http://127.0.0.1:17890");
    let mut jar = CookieJar::default();
    let mut probe = Probe::new(
        &mut exec,
        provider("antigravity"),
        Some(&settings),
        &mut jar,
        Consent::Quiet,
    );

    let output = probe
        .command(
            "sh",
            &[
                "-c",
                "printf '%s|%s|%s|%s' \"$HTTP_PROXY\" \"$HTTPS_PROXY\" \"$http_proxy\" \"$https_proxy\"",
            ],
            Duration::from_secs(5),
        )
        .unwrap();
    assert!(output.success);
    assert_eq!(output.stdout, "|||");

    let response = probe
        .http(Request::get("https://example.com/usage"))
        .unwrap();
    assert_eq!(response.status, 200);
    let log = std::fs::read_to_string(&log).unwrap();
    assert_eq!(log.trim(), "env:|||");
}

#[test]
fn usage_provider_config_retains_proxy_for_codex_and_antigravity() {
    use secrecy::ExposeSecret;

    let mut config: UsageConfig = toml::from_str(
        r#"
[providers.codex]
proxy = "http://127.0.0.1:7890"

[providers.antigravity]
proxy = "https://proxy.example:8443"
"#,
    )
    .unwrap();
    let unknown = config.retain_known();
    assert!(unknown.is_empty(), "unexpected unknown paths: {unknown:?}");
    assert_eq!(
        config
            .settings(provider("codex"))
            .and_then(|settings| settings.get("proxy"))
            .map(|value| value.expose_secret()),
        Some("http://127.0.0.1:7890")
    );
    assert_eq!(
        config
            .settings(provider("antigravity"))
            .and_then(|settings| settings.get("proxy"))
            .map(|value| value.expose_secret()),
        Some("https://proxy.example:8443")
    );
}

#[test]
fn antigravity_invalid_proxy_is_reported() {
    let settings = ProviderSettings::default().with("cli_path", "agy").with(
        "proxy",
        "socks5://sentinel-user:sentinel-secret@127.0.0.1:17890",
    );
    let mut exec = Exec::Local;
    let mut jar = CookieJar::default();
    let antigravity = provider("antigravity");
    let mut probe = Probe::new(
        &mut exec,
        antigravity,
        Some(&settings),
        &mut jar,
        Consent::Ask { browsers: false },
    );

    let result = antigravity.service().fetch(&mut probe);
    assert!(
        matches!(result, Some(Err(Error::InvalidProxy))),
        "{result:?}"
    );
}
