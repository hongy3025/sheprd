# Usage provider-local proxy

状态：用户已于 2026-11-20 批准。实现须等待用户确认当前分支开发完成。

## 意图与范围

让用户为 Codex 和 Antigravity usage 分别设置本机代理，以便 Sheprd 从独立启动的进程访问服务；未配置时保留现有代理环境变量行为。

本次涵盖 Codex 的 usage HTTP 请求和 Antigravity 的 `agy` usage 命令。只影响本机 usage，不把本机配置代理传给 SSH 或 WSL 主机。

## 当前实现

- Codex provider 从本机 Codex/OpenCode 登录文件读取令牌，再由 `Probe::body` 直接请求 ChatGPT usage API。它不运行 Codex CLI。
- Antigravity provider 通过 `Probe::command` 运行 `agy --version` 和 `agy -p /usage --output-format json`。
- 本机 HTTP 使用 `ureq`，默认读取 Sheprd 进程的代理环境变量；本机子进程继承 Sheprd 环境。
- 远端 HTTP 使用远端 shell 的 `curl`，远端子进程继承远端环境。远端代理配置继续由远端环境提供。
- `UsageConfig` 的 provider 设置已存为 `SecretString`，并由 provider 元数据声明允许的设置名。

## Approaches

1. **每个 provider 可选 `proxy` 设置，选定方案。** Codex 和 Antigravity 使用相同的 provider 设置约定；请求和命令的共用执行层负责应用代理。只改变明确配置的 provider。
2. **全局 `[usage] proxy`，未选。** 更少配置，但会把所有 provider 的网络流量导向同一代理，超出当前 Codex/Antigravity 的需要。
3. **只依赖进程环境，未选。** 当前代码已支持环境代理，但不能从 Sheprd 配置面板独立配置 provider，也不能复现仅写在 `cx.cmd` 里的子进程环境。

## 已确认的设计决策

- Codex 与 Antigravity 都提供可选的 provider 级 `proxy` 配置，例如：

  ```toml
  [usage.providers.codex]
  proxy = "http://127.0.0.1:7890"

  [usage.providers.antigravity]
  proxy = "http://127.0.0.1:7890"
  ```

- 设置只作用于本机。配置代理覆盖这两个 provider 的本机出站请求；不设置时保留现有 `HTTP_PROXY`、`HTTPS_PROXY` 和 `NO_PROXY` 环境行为。
- SSH/WSL 主机不读取、接收或使用本机配置的代理，继续使用其自身环境代理。远端 `127.0.0.1` 语义不被本机设置改变。
- 不新增独立代理 UI；沿用 usage provider 的本地配置文件与设置约定。

## 实现约束与安全

- 代理是传输选项，不是 usage 请求凭据；不得改变 `Probe` 对请求执行位置的判定，也不得把远端凭据拉回本机。
- 本机 Codex HTTP 请求要使用显式代理；本机 Antigravity 的 `agy` 命令要收到对应的代理环境。两者配置必须一致。
- 不通过修改进程全局环境变量来设置代理，避免影响其他 provider 或并发工作；仅作用于本机对应 provider 的请求/子进程。
- 保留 `NO_PROXY` 绕过规则。配置的代理 URL 若无效或不受支持，应报告明确错误，不能静默回退到直连。
- 代理 URL 作为敏感配置处理；不得写入日志、错误文本、进程参数或诊断内容。
- 支持的代理 URL 方案必须同时适用于 `ureq` 与 `agy` 子进程代理环境；具体校验规则由实现阶段按依赖能力确定并在配置文档中说明。

## 目标与非目标

**目标**

- 用户可在 provider 表中为 Codex 与 Antigravity 独立设置代理。
- 设置对本机 Codex HTTP 请求和 Antigravity usage 命令都生效。
- 未设置时保持现有环境代理行为；远端行为不变。
- 配置无效时错误可见且不泄漏代理 URL 凭据。

**非目标**

- 不改变 Codex 登录发现、usage API、Antigravity CLI 或额度解析。
- 不代理其他 usage providers，不新增全局代理或系统代理配置。
- 不把本机代理配置传到 SSH/WSL，不增加远端代理管理。
- 不修改 `cx.cmd`、Herdr 或 Sheprd 的启动方式。

## 验收

1. 配置 Codex 代理后，本机 Codex HTTP 请求经过指定代理；未配置时继续遵循现有进程代理环境。
2. 配置 Antigravity 代理后，`agy` 子进程只在本次 provider 操作中获得代理环境；不配置时继承原环境。
3. 配置 Codex 代理不改变 Antigravity 或其他 provider 的代理；反之亦然。
4. 显式代理设置不改变本机 HTTP 请求的 `NO_PROXY` 行为。
5. SSH/WSL provider 请求不携带本机代理配置，继续使用远端代理环境。
6. 无效代理配置给出可见错误；包含代理用户名/密码的配置值不出现在诊断、错误或命令参数中。
7. 更新 `docs/usage-providers.md` 和示例配置；为配置解析、本机 HTTP 与命令环境覆盖、隔离和远端不变行为添加确定性测试。

## 待实现方决定

- 在共享 provider 元数据中如何声明 `proxy` 可用，避免 Codex 与 Antigravity 重复定义同一设置语义。
- 使用 `ureq`/`curl` 均支持的代理 URL 子集，并在文档中写清楚。
- 选用能证明本机、并发隔离与凭据不泄漏的最窄验证方式。
