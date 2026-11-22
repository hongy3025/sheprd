# 定制版 Herdr 远端机器强制升级命令设计（`herdr machine upgrade`）

日期：2026-11-22  
目标仓库：`D:\AI\herdr`（本文档作为跨会话实施规格保留于 `D:\AI\sheprd`）

## 1. 背景与目标

定制版 `herdr` 已支持通过同目录下的 `herdr-dist.zip`（或 `HERDR_REMOTE_BINARY`）为远端 SSH 机器安装定制服务端，但当前仅在以下场景触发：

- `herdr machine add <ssh-target> --force`：用于新增机器时强制重装；若机器已保存在配置目录中，会被重复名称/目标校验拒绝，需要先 `machine remove` 再 `machine add --force`。
- `herdr --remote <ssh-target>`：仅在远端版本不兼容时提示安装/升级，若远端已兼容则直接连接，无法用于同版本覆盖重装或日常一键升级。

目标：在 `herdr machine` 子命令组下提供 **`herdr machine upgrade`**，支持单台或全量已保存机器的强制升级，并提供 `--yes`（`-y`）非交互自动确认模式以支持全自动升级。

## 2. 命令设计

### 2.1 CLI 语法

```text
herdr machine upgrade <label-or-id> [--yes|-y]
herdr machine upgrade --all [--yes|-y]
```

- `<label-or-id>`：已保存机器的标签（`label`）或 ID（`profile_id`），复用现有 `super::target::resolve_machine(&catalog.ssh, selector)` 解析。
- `--all`：依次升级目录中所有已保存且启用的 SSH 机器；单台失败时打印错误并继续升级其余机器，最终任一失败返回退出码 `1`。
- `--yes` / `-y`：非交互自动确认模式，跳过终端 TTY 检查与 `[Y/n]` / `[y/N]` 确认提示，适用于自动化脚本或由 Agent 代执行升级。
- 复用现有 `SavedSshSetup::prepare(&profile.session, true, auto_approve)` 的 `force_install = true` 路径：
  - 不更改机器的 `label`、`target`、`session` 与 `enabled` 状态；
  - 不需要用户先 `machine remove` 再 `machine add`。

### 2.2 交互与安全前提

- **未传 `--yes` 时**：必须运行在交互式终端（`std::io::stdin().is_terminal()`），与 `herdr machine reconnect` 保持一致；非交互环境返回退出码 `2` 并提示使用交互终端或传 `--yes`。
- **传入 `--yes` 时**：允许在非交互环境中执行，自动批准以下操作：
  - 确认安装包来源（优先 `herdr-dist.zip` / `HERDR_REMOTE_BINARY`）；
  - 若远端已有对应 session 的 `herdr server` 在运行，自动批准停止旧服务端并重启为新安装的定制版 `herdr`。

## 3. 实现方案（基于 `D:\AI\herdr` 现有代码）

### 3.1 修改文件清单

1. `src/cli/machine.rs` 与 `src/cli/spec/machine.rs`
   - 在 `HELP` 与 clap spec 中补充 `herdr machine upgrade (<label-or-id> | --all) [--yes|-y]`。
   - 参数解析拒绝同时传入 `<label-or-id>` 与 `--all`，以及二者均缺失的情况（返回退出码 `2`）。
   - 未传 `--yes` 时检查 `std::io::stdin().is_terminal()`；传入 `--yes` 时跳过该检查，并以 `auto_approve = true` 调用 `setup.prepare(&profile.session, true, true)`。
   - 若返回新的 `Some(metadata)`，更新 `SshMetadataCache`。

2. `src/remote/attach.rs`
   - 为 `SavedSshSetup::prepare` 及内部 `prepare_discovered_remote_herdr`、`prepare_windows_remote_herdr`、`confirm_remote_install`、`confirm_remote_install_with_running_server` 增加 `auto_approve: bool` 参数（既有调用方默认传 `false`，保持原交互确认行为不变）。
   - 当 `auto_approve == true` 时：
     - `confirm_remote_install` 跳过 `io::stdin().is_terminal()` 与 `[Y/n]` 读取，直接返回 `Ok(())`；
     - `confirm_remote_install_with_running_server` 在探测失败或需要重启远端服务时跳过交互确认，直接批准继续安装并返回 `Ok(stop_after_install_approved)`（使新安装完成后自动重启远端 `herdr server`）。

### 3.2 边界情况与错误处理

- **无 `herdr-dist.zip` 且无 `HERDR_REMOTE_BINARY`**：`require_custom_source_for_force(true, ...)` 会明确拒绝并提示缺少定制包，不会静默回退到官方公网清单覆盖定制版。
- **远端 `herdr-dist.zip` 与当前 `herdr.exe` 版本/协议不一致**：`DistArchive::open` 校验失败并明确报错提示重新打包 `herdr-dist.zip`。
- **SSH 认证失败或连接中断**：沿用 `print_saved_ssh_error_hint` 提示用户排查 SSH 信任与认证，不破坏本地已保存的机器条目。
- **远端原会话保护**：远端既有终端与 agent 会话保留（按 `attach.rs` 既定策略重启/接管服务端）。

## 4. 测试与验证计划

1. 在 `src/cli/machine.rs` 与 `src/remote/attach.rs` 补充单元测试：
   - `--yes` / `-y` 与 `--all` 参数解析及冲突/缺失校验；
   - `auto_approve = true` 时非交互路径不再因 `!stdin().is_terminal()` 拒绝，并保持 `auto_approve = false` 的既有拒绝行为不变。
2. 构建与打包验证（在 `D:\AI\herdr`）：
   - 运行 `cargo test cli::` 与 `cargo test remote::attach`；
   - 重新构建 release 版 `herdr.exe` 与 `herdr-dist.zip`；
   - 回到 `D:\AI\sheprd` 执行 `build-portable.cmd` 并同步到 `D:\app\devel\sheprd-portable`；
   - 执行 `D:\app\devel\sheprd-portable\herdr.exe machine upgrade --all --yes` 验证全自动升级所有已保存远端机器。
