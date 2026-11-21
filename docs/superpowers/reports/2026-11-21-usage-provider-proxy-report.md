# Usage Provider-Local Proxy 开发报告

- **日期**: 2026-11-21
- **分支**: `feat/usage-provider-proxy`（基于 `875f34ee6`）
- **设计文档**: `docs/superpowers/specs/2026-11-20-usage-provider-proxy-design.md`
- **实现计划**: `docs/superpowers/plans/2026-11-20-usage-provider-proxy.md`

## 1. 摘要

为 Codex 与 Antigravity 的本机 usage 查询实现了独立、可配置的 provider 级 `proxy` 设置，使从桌面启动的独立 Sheprd 进程可为两个服务分别指定本地出站代理，同时保持以下边界：

1. **未配置不变**：未设置（或为空白）时，继续沿用 `ureq` 默认的进程环境变量代理解析与子进程继承行为。
2. **本机/provider 双重隔离**：不修改进程全局环境变量，也不影响其他 provider；SSH 与 WSL 远端主机不读取、不接收本机配置代理，继续使用远端环境。
3. **严格校验与凭据脱敏**：仅支持 `http://` 与 `https://` 代理 URL；配置无效时返回无载荷的 `Error::InvalidProxy`，绝不在日志、错误文本、`Debug` 输出或进程参数中暴露凭据。

## 2. 核心实现与变更文件

### 2.1 本机代理校验与传输隔离（`sheprd-usage`）

| 文件 | 变更内容 |
| --- | --- |
| `crates/sheprd-usage/src/probe/proxy.rs` | 新增模块（146 行）：实现 `parse_proxy`、`decode_percent`、`env_no_proxy`、`build_ureq_proxy`、`command_proxy_value`、`local_command` |
| `crates/sheprd-usage/src/probe.rs` | 新增 `Probe::local_proxy`、`Probe::local_ureq_proxy`、`local_agent_config`；将校验后的本机代理接入 `Probe::http`、`Probe::exchange` 与 `Probe::command`（文件共 943 行，未超 1,000 行上限） |
| `crates/sheprd-usage/src/error.rs` | 新增无载荷错误变体 `Error::InvalidProxy`（`"Configured usage proxy URL is invalid."`） |

关键实现细节：

- **URL 校验与凭据处理**（`crates/sheprd-usage/src/probe/proxy.rs`）：
  - 在 `.trim()` 前先拒绝原始字符串中的 ASCII 控制字符，随后拒绝空串、内部空白、非 `http`/`https` 协议（如 `socks5://`）、缺失主机或 `0` 端口、非根路径（`url.path() != "/"`）、query、fragment，以及非法 percent-encoding 或非 UTF-8 解码结果。
  - 解码 percent-encoded 用户名/密码时保持字面量 `+` 不被转为空格，并在仅配置密码（`:secret@host`）时显式传入空用户名，避免 `ureq::ProxyBuilder` 在缺少 `username` 时丢弃 `password`。
  - 使用 `Zeroizing` 持有解码和规范化后的凭据字符串；`Error::InvalidProxy` 不携带任何原始输入。
- **本机 HTTP 与 `NO_PROXY`**：
  - `Probe::local_proxy` 仅在 `Exec::Local` 下读取 `ProviderSettings.get("proxy")`，将空串及纯空格视作未配置。
  - 显式配置代理时，`build_ureq_proxy` 读取继承的 `NO_PROXY` / `no_proxy`（按逗号拆分并去空格）逐项注册到 `ureq::ProxyBuilder::no_proxy`，并传入 `local_agent_config`。
  - 未配置代理时，`local_agent_config` 不调用 `.proxy(...)`，完整保留 `ureq::Agent::config_builder()` 默认的 `Proxy::try_from_env()` 行为。
- **本机子进程命令环境**：
  - `local_command` 仅在待执行的 `Command` 上移除 `ALL_PROXY`（Unix 下同时移除 `all_proxy`）并设置 `HTTP_PROXY`/`HTTPS_PROXY`（Unix 下同时设置小写 `http_proxy`/`https_proxy`），保持继承的 `NO_PROXY` 不变，且绝不通过命令行参数传递代理 URL。
  - `Exec::Remote` 分支（`Shell::http`、`Shell::exchange`、`Shell::run`）完全不读取或传入本机 `proxy`。

### 2.2 共享元数据、Provider 接入与文档/本地化

| 文件 | 变更内容 |
| --- | --- |
| `crates/sheprd-usage/src/service.rs` | 新增 `Setting::proxy(message)` 构造器，统一定义 `"proxy"` 与空环境变量列表 `&[]` |
| `crates/sheprd-usage/src/providers/codex.rs` | 为 `Codex::META` 声明 `Setting::proxy("usage-codex-setting-proxy")` |
| `crates/sheprd-usage/src/providers/antigravity.rs` | 为 `Antigravity::META` 保留 `cli_path` 并追加 `Setting::proxy("usage-antigravity-setting-proxy")`；将 `agy --version` 的 `Err(Error::InvalidProxy)` 显式透传，避免被 `.ok()?` 吞掉 |
| `crates/sheprd-gpui/src/i18n/errors/usage.rs` | 为 `Localize for Error` 补全 `Self::InvalidProxy => fl!("error-usage-invalid-proxy")` |
| `crates/sheprd-gpui/locales/en-US/error-usage.ftl` | 新增 `error-usage-invalid-proxy` |
| `crates/sheprd-gpui/locales/en-US/usage-codex.ftl` | 新增 `usage-codex-setting-proxy` 说明文档 |
| `crates/sheprd-gpui/locales/en-US/usage-antigravity.ftl` | 新增 `usage-antigravity-setting-proxy` 说明文档 |
| `crates/sheprd-gpui/src/i18n/errors/tests/usage.rs` | 将 `Error::InvalidProxy` 加入 `every_variant_has_a_message` 校验列表 |
| `crates/sheprd-gpui/config-gpui.example.toml` | 通过 `HERDR_BLESS_EXAMPLE=1` 测试重新生成 provider 示例配置块 |
| `docs/usage-providers.md` | 新增 `Local proxy for Codex and Antigravity` 章节，说明格式、回退、`NO_PROXY` 与远端隔离 |

### 2.3 测试覆盖

在 `crates/sheprd-usage/src/tests.rs` 引入 `crates/sheprd-usage/src/tests/proxy.rs`，新增 9 个确定性测试（8 个跨平台测试 + 1 个 `#[cfg(unix)]` 测试），全程不修改父进程环境变量：

- `configured_local_http_request_uses_provider_proxy`：本地假 HTTP CONNECT 代理 + 不可解析域名（`nonexistent.invalid`）+ percent-encoded 凭据，验证 `CONNECT` 隧道建立、解码后的 `Proxy-Authorization: Basic` 请求头以及内层 HTTP 转发。
- `configured_proxy_overrides_local_command_env`：校验 `local_command` 的环境变量注入与真实子进程执行后的 `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY`，并断言父进程环境未被修改。
- `configured_no_proxy_bypasses_explicit_proxy`：验证匹配的 `NO_PROXY` 条目（含通配/域匹配）绕过显式代理直连本地源站且不触碰诱饵代理监听端口。
- `unset_provider_proxy_keeps_environment_fallback`：验证未配置或空白 `proxy` 时 `local_proxy` 为 `None`，`local_agent_config` 与默认 `ureq::Agent` 代理配置一致且 `local_command` 不写入代理变量。
- `invalid_proxy_has_redacted_error`：覆盖多种非法 scheme、缺 host、0 端口、path/query/fragment、控制字符/换行、非法 `%` 转义与非 UTF-8 凭据，验证返回 `Error::InvalidProxy` 且 `Display`/`Debug` 不包含哨兵凭据；同时覆盖合法 `https://`、默认端口、末尾 `/`、IPv6 `[::1]` 与 percent-encoded 凭据解析。
- `configured_proxy_is_provider_scoped`：验证 Codex HTTP、Antigravity 命令及未配置代理的 Claude 互不串用。
- `usage_provider_config_retains_proxy_for_codex_and_antigravity`：验证 TOML 解析及 `UsageConfig::retain_known()` 保留两者的 `proxy` 配置且无未知键告警。
- `antigravity_invalid_proxy_is_reported`：验证 `Antigravity::fetch` 遇到无效代理配置时返回 `Some(Err(Error::InvalidProxy))`。
- `remote_probe_does_not_receive_local_proxy`（`#[cfg(unix)]`）：通过本地 `sh` 模拟 `Exec::Remote` 与假 `curl`，验证远端命令和远端 HTTP 请求均不会收到本机 `proxy`。

## 3. 关键设计裁决（Rulings）

1. **同步更新 `sheprd-gpui` 错误本地化**：因 `Localize for sheprd_usage::Error` 要求穷举全部变体，新增 `Error::InvalidProxy` 时一并补齐 `error-usage.ftl` 与对应单测。
2. **显式透传 Antigravity `--version` 阶段 `InvalidProxy`**：防止前置版本探测的 `.ok()?` 静默吞掉配置错误，符合设计文档对“配置无效时不得静默失败”的要求。
3. **空字符串与纯空格视作未配置**：与 `cli_path`、`base_url` 等设置及示例配置 `# proxy = ""` 占位习惯保持一致。
4. **以 `ureq` 3 实际 `CONNECT` 协议构建测试代理**：经核实 `ureq` 3 的 `ConnectProxyConnector` 对 `http://` 代理先发 `CONNECT <host>:<port> HTTP/1.1` 再经隧道转发请求，测试据此验证代理路由与非本地 DNS 解析。
5. **Unix 同步覆写小写代理并清除 `ALL_PROXY`/`all_proxy`**：避免父进程继承的小写 `http_proxy`/`https_proxy` 或 `ALL_PROXY` 在 `curl` 等 CLI 工具中压过配置值；Windows 环境变量不区分大小写，清除 `ALL_PROXY` 后设置 `HTTP_PROXY`/`HTTPS_PROXY`。
6. **保留非目标与现有边界**：保持 Antigravity 原有 `/bin/sh` 触发机制及 `#320` 同意门槛不变；仅对 `codex` 与 `antigravity` 启用 `proxy`；沿用 `ureq` 原生 `NO_PROXY` 匹配与 `zh-Hans` 回退 `en-US` 机制。

## 4. 验收与检查记录

### 本机验证（Windows，全部在最后一次代码修改后执行通过）

- `cargo test --locked -p sheprd-usage --lib`：**421 passed, 0 failed, 2 ignored**（2 个 ignored 为需真实环境资源的 `live_local_usage` 与 `live_wsl_usage`；`tests::proxy` 在 Windows 启用的 8 个用例全部通过）。
- `cargo test --locked -p sheprd-gpui --lib usage::tests`：**103 passed, 0 failed**（含 `example_config_documents_every_provider_setting`、`every_setting_names_a_message_without_arguments`、`every_message_herdr_usage_names_exists_with_its_arguments`、`en_xa` 伪本地化检查）。
- `cargo test --locked -p sheprd-gpui --lib i18n::errors::tests::usage`：**1 passed, 0 failed**。
- `cargo clippy --locked -p sheprd-usage --all-targets -- -D warnings`：通过（0 警告）。
- `cargo clippy --locked -p sheprd-gpui --lib --tests -- -D warnings`：通过（0 警告）。
- `cargo fmt --all -- --check`、`python scripts/check-i18n.py`（7684 messages in 375 files valid and used）、`python scripts/check-sorted.py`、`git diff --check`：均通过。
- **独立全量代码审查**：由全新上下文审查 subagent 完成 diff 评审，结论为 **Ready to merge: Yes**（0 Critical、0 Important、4 Minor 改进建议）。

### 延迟至 CI / 后续可选项

- **Unix CI 验证项**：`#[cfg(unix)]` 下的 `remote_probe_does_not_receive_local_proxy` 及 `local_command` 中 `http_proxy`/`https_proxy`/`all_proxy` 分支、全工作区测试与跨平台 lint 交由 Linux/macOS CI 验证。
- **审查记录的可选 Minor 项（未阻塞合入）**：
  - 在 `configured_proxy_overrides_local_command_env` 中直接断言 `ALL_PROXY`（及 Unix `all_proxy`/`http_proxy`/`https_proxy`）的 `get_envs()` 项；
  - 在 `invalid_proxy_has_redacted_error` 中补充 `command_proxy_value` 断言及 `HTTP://`、`:secret@`、`user@`、`//` 用例；
  - 为 `decode_percent` 中 `mem::take` 前的 `from_utf8` 校验补一行说明注释；
  - 在 `usage_provider_config_retains_proxy_for_codex_and_antigravity` 中补充校验未启用 `proxy` 的其他 provider 会拒绝 `proxy` 键。
