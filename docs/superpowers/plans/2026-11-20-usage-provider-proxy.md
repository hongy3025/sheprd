# Usage Provider Proxy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users configure independent local proxies for Codex and Antigravity usage without changing other providers or remote hosts.

**Architecture:** Keep proxy values in each provider's existing secret settings table. Apply an explicitly configured proxy in the shared `Probe` layer to that provider's local HTTP requests and local child commands; leave remote paths and the existing environment-proxy fallback unchanged.

**Tech Stack:** Rust, `ureq` 3, `curl`/child-process environment, TOML provider settings, Fluent.

**Spec:** `docs/superpowers/specs/2026-11-20-usage-provider-proxy-design.md`

## Global Constraints

- Only affects local usage; the local proxy configuration is not sent to SSH or WSL hosts.
- Proxy configuration must not change `Probe`'s request-placement decision or move remote credentials to the local host.
- Do not mutate process-global environment variables.
- Keep `NO_PROXY` bypass behavior when an explicit local proxy is configured.
- Treat proxy URLs as secrets; do not include them in logs, error messages, process arguments, or diagnostics.
- Do not add dependencies, runtimes, or a global usage proxy.
- Do not implement until the user confirms the current branch development is complete.

## Review Focus

1. Invalid schemes and malformed proxy URLs, including URLs with credentials: return a typed error that never echoes the value. Test: `invalid_proxy_has_redacted_error`.
2. `NO_PROXY` matching the target: bypass the configured proxy. Test: `configured_no_proxy_bypasses_explicit_proxy`.
3. No provider proxy configured: retain the current environment-based proxy behavior. Test: `unset_provider_proxy_keeps_environment_fallback`.
4. A local provider proxy must not leak into SSH/WSL commands or alter remote proxy behavior. Test: `remote_probe_does_not_receive_local_proxy`.
5. Configuring one provider must not affect another provider or mutate the app's process environment. Test: `configured_proxy_is_provider_scoped`.

---

### Task 1: Add isolated local proxy handling to the usage probe

**Files:**
- Modify: `crates/sheprd-usage/src/probe.rs`
- Modify: `crates/sheprd-usage/src/error.rs`
- Modify: `crates/sheprd-usage/src/tests.rs`
- Create: `crates/sheprd-usage/src/tests/proxy.rs`

**Interfaces:**
- Consumes: `Probe`'s `ProviderSettings`; provider settings store values as `SecretString`.
- Produces: `Probe` applies the declared `"proxy"` setting only when executing local HTTP requests and local provider commands.
- Produces: Add `crate::Error::InvalidProxy` with a fixed display message and no proxy-value payload.
- `http_local` accepts an optional configured proxy; when absent it builds the current default `ureq` agent.
- Local command execution sets proxy environment variables on that child only. Remote HTTP and command execution remain unchanged.

- [ ] **Step 1: Write focused failing transport tests**

Add `mod proxy;` to `crates/sheprd-usage/src/tests.rs`. In `tests/proxy.rs`, add:

- `configured_local_http_request_uses_provider_proxy`: use a local fake HTTP proxy and a non-resolving target hostname; assert the proxy receives the absolute request URL and supplies the response.
- `configured_proxy_overrides_local_command_env`: inspect the command's explicit `HTTP_PROXY` and `HTTPS_PROXY` overrides; assert inherited `NO_PROXY` remains untouched and the parent process environment was not modified.
- `configured_no_proxy_bypasses_explicit_proxy`: test the proxy builder with a matching `NO_PROXY` entry and assert that the target is bypassed.
- `unset_provider_proxy_keeps_environment_fallback`: assert no explicit proxy is installed when the setting is absent, leaving `ureq`'s existing default configuration intact.
- `invalid_proxy_has_redacted_error`: pass an invalid proxy URL containing a sentinel credential; assert `Error::InvalidProxy` and that neither its display nor debug output contains the sentinel.
- On Unix, `remote_probe_does_not_receive_local_proxy`: run a local shell through `Shell::start`, invoke `Probe::command` with a local provider setting, and assert the setting is absent from the remote command environment.
- `configured_proxy_is_provider_scoped`: construct separate Codex and Antigravity settings and verify each probe uses only its own value.

- [ ] **Step 2: Run a failing transport test**

Run: `cargo test -p sheprd-usage --lib configured_local_http_request_uses_provider_proxy`

Expected: FAIL because local HTTP execution does not yet apply `ProviderSettings.proxy`.

- [ ] **Step 3: Implement local proxy selection and validation**

In `probe.rs`, add a private `Probe::local_proxy(&self) -> Option<&SecretString>` that returns the configured `proxy` only for `Exec::Local`. Accept `http://` and `https://` proxy URLs with optional percent-encoded user/password and optional default port; reject other schemes and URLs with a path, query, or fragment. Build the `ureq::Proxy` from the parsed URI, applying the inherited comma-separated `NO_PROXY` entries, and map parse failures to `Error::InvalidProxy` without retaining the original text.

Pass the parsed proxy to the local `ureq` agent in `http_local`; leave the agent's existing environment-based default untouched when no setting is configured. Use the same validation before a local provider command, then set `HTTP_PROXY` and `HTTPS_PROXY` on that `Command` only. Keep the inherited `NO_PROXY`; do not mutate process-global variables. Do not pass the setting into `Shell::http`, `Shell::exchange`, or remote `Shell::run`.

- [ ] **Step 4: Run the usage proxy tests**

Run: `cargo test -p sheprd-usage --lib proxy`

Expected: PASS for configured routing, environment fallback, `NO_PROXY`, provider isolation, remote isolation, and redacted invalid-value behavior.

### Task 2: Enable the shared setting for Codex and Antigravity

**Files:**
- Modify: `crates/sheprd-usage/src/providers/codex.rs`
- Modify: `crates/sheprd-usage/src/providers/antigravity.rs`
- Modify: `crates/sheprd-usage/src/tests/proxy.rs`
- Modify: `crates/sheprd-gpui/locales/en-US/usage-codex.ftl`
- Modify: `crates/sheprd-gpui/locales/en-US/usage-antigravity.ftl`
- Modify: `docs/usage-providers.md`
- Regenerate section: `crates/sheprd-gpui/config-gpui.example.toml`

**Interfaces:**
- Consumes: Task 1's `Probe` transport behavior, activated by a provider declaring a `"proxy"` setting.
- Produces: `[usage.providers.codex].proxy` and `[usage.providers.antigravity].proxy`, both config-only settings with `env = &[]`.
- Produces: Localized setting help and provider documentation that state the setting is optional and local-only.

- [ ] **Step 1: Write the failing provider-config test**

Add `usage_provider_config_retains_proxy_for_codex_and_antigravity` to `tests/proxy.rs`. Parse a TOML config containing both provider tables and `proxy` values, call `UsageConfig::retain_known`, and assert both values remain and no unknown-setting paths are returned.

- [ ] **Step 2: Run the failing provider-config test**

Run: `cargo test -p sheprd-usage --lib usage_provider_config_retains_proxy_for_codex_and_antigravity`

Expected: FAIL because neither provider declares the `proxy` setting yet.

- [ ] **Step 3: Declare the setting and document it**

Declare `proxy` in each provider's `Meta.settings` with an empty environment-variable list; retain Antigravity's existing `cli_path` setting. Add `usage-codex-setting-proxy` and `usage-antigravity-setting-proxy` help messages explaining the accepted `http://` or `https://` URL format, optional percent-encoded credentials, that the setting applies only on the local host, and that it is not propagated to SSH/WSL.

Document proxy configuration and fallback behavior in `docs/usage-providers.md`. Regenerate the provider settings section in `config-gpui.example.toml` through its existing blessing test; do not hand-edit the generated block.

- [ ] **Step 4: Regenerate and verify the provider config example**

Run: `HERDR_BLESS_EXAMPLE=1 cargo test -p sheprd-gpui --lib example_config_documents_every_provider_setting`

Expected: PASS and the generated example lists `proxy` under both providers.

Then run: `cargo test -p sheprd-gpui --lib example_config_documents_every_provider_setting`

Expected: PASS without the blessing variable.

- [ ] **Step 5: Run focused provider and documentation checks**

Run: `cargo test -p sheprd-usage --lib proxy`

Expected: PASS, including provider setting retention and all local/remote proxy behaviors.

Run: `cargo test -p sheprd-usage --lib every_provider_is_registered_once_with_its_icon`

Expected: PASS, including validation that each setting message uses the provider's expected Fluent key prefix.

Run: `git diff --check`

Expected: no whitespace errors.

Do not commit or begin implementation until the user confirms the active branch development is complete.
