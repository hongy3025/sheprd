# 定制版 Herdr 远端机器强制升级命令设计（`herdr machine upgrade`）

日期：2026-11-22  
目标仓库：`D:\AI\herdr`（本文档作为跨会话实施规格保留于 `D:\AI\sheprd`）

## 1. 背景与目标

定制版 `herdr` 已支持通过同目录下的 `herdr-dist.zip`（或 `HERDR_REMOTE_BINARY`）为远端 SSH 机器安装定制服务端，但当前仅在以下场景触发：

- `herdr machine add <ssh-target> --force`：用于新增机器时强制重装；若机器已保存在配置目录中，会被重复名称/目标校验拒绝，需要先 `machine remove` 再 `machine add --force`。
- `herdr --remote <ssh-target>`：仅在远端版本不兼容时提示安装/升级，若远端已兼容则直接连接，无法用于同版本覆盖重装或日常一键升级。

目标：在 `herdr machine` 子命令组下新增 **`herdr machine upgrade <label-or-id>`**，直接针对已保存的远端机器执行强制升级/重装，并自动重启远端 `herdr` 服务端。

## 2. 命令设计

### 2.1 CLI 语法

```text
herdr machine upgrade <label-or-id>
```

- `<label-or-id>`：已保存机器的标签（`label`）或 ID（`profile_id`），复用现有 `super::target::resolve_machine(&catalog.ssh, selector)` 解析。
- 复用现有 `SavedSshSetup::prepare(&profile.session, true)` 的 `force_install = true` 路径：
  - 不更改机器的 `label`、`target`、`session` 与 `enabled` 状态；
  - 不需要用户先 `machine remove` 再 `machine add`。

### 2.2 交互与安全前提

- 必须运行在交互式终端（`std::io::stdin().is_terminal()`），与 `herdr machine reconnect` 保持一致；非交互环境返回退出码 `2` 并提示在交互式终端中运行。
- 强制安装会触发既有确认提示：
  - 确认安装包来源（优先 `herdr-dist.zip` / `HERDR_REMOTE_BINARY`）；
  - 若远端已有对应 session 的 `herdr server` 在运行，提示确认停止旧服务端并重启为新安装的定制版 `herdr`。

## 3. 实现方案（基于 `D:\AI\herdr` 现有代码）

### 3.1 修改文件清单

1. `src/cli/machine.rs`
   - 在 `HELP` 文本中补充 `herdr machine upgrade <label-or-id>` 及说明。
   - 在 `run_machine_command` 的 `match` 中新增分支：
     `Some("upgrade") => upgrade(&args[1..])`
   - 新增 `fn upgrade(args: &[String]) -> std::io::Result<i32>`：
     1. 校验参数为单个位置参数 `[selector]`，否则输出 `usage: herdr machine upgrade <label-or-id>` 并返回 `2`。
     2. 加载目录 `let catalog = load_catalog()?;`，调用 `super::target::resolve_machine(&catalog.ssh, selector)` 解析出 `profile`（找不到则返回 `2`）。
     3. 检查 `std::io::stdin().is_terminal()`，非交互终端直接报错并返回 `2`。
     4. 调用 `crate::remote::SavedSshSetup::connect(&profile.target)` 并执行 `.and_then(|setup| setup.prepare(&profile.session, true))`：
        - 失败时输出错误并调用 `crate::remote::print_saved_ssh_error_hint(&error, &profile.target)`，返回 `1`。
     5. 若返回新的 `Some(metadata)`，更新缓存：
        `crate::client::endpoint::SshMetadataCache::new(profile.id.as_str(), &profile.target, &profile.session)?.store(&metadata);`
     6. 打印完成提示：
        - `Upgraded remote Herdr on {id} ({target}). Remote server is ready.`
        - `Open Herdr clients reconnect automatically.`
        并返回 `0`。

2. 底层复用（无需修改 `src/remote/attach.rs`）
   - `SavedSshSetup::prepare(&profile.session, true)` 内部已调用：
     - `prepare_discovered_remote_herdr(..., force_install = true)`
     - `ensure_remote_server_ready(..., force_install = true, ...)`
   - 会自动完成：
     - 校验同目录 `herdr-dist.zip`（或 `HERDR_REMOTE_BINARY`）存在且与当前 `herdr.exe` 的 `build_identity`、`version`、`protocol` 匹配；
     - 按目标平台（Linux x86_64/ARM64、macOS、Windows 含 `conpty` 运行时）上传并原子替换远端二进制；
     - 重启该 `profile.session` 的远端 `herdr server`。

### 3.2 边界情况与错误处理

- **无 `herdr-dist.zip` 且无 `HERDR_REMOTE_BINARY`**：`require_custom_source_for_force(true, ...)` 会明确拒绝并提示缺少定制包，不会静默回退到官方公网清单覆盖定制版。
- **远端 `herdr-dist.zip` 与当前 `herdr.exe` 版本/协议不一致**：`DistArchive::open` 校验失败并明确报错提示重新打包 `herdr-dist.zip`。
- **SSH 认证失败或连接中断**：沿用 `print_saved_ssh_error_hint` 提示用户排查 SSH 信任与认证，不破坏本地已保存的机器条目。
- **远端原会话保护**：远端既有终端与 agent 会话保留（仅在用户确认后按 `attach.rs` 既定策略重启/接管服务端）。

## 4. 测试与验证计划

1. 在 `src/cli/machine.rs` 单元测试模块中新增测试：
   - 参数为空或多余参数时返回用法错误与退出码 `2`；
   - `HELP` 文本包含 `herdr machine upgrade <label-or-id>`；
   - 非交互终端 / 不存在的 `<label-or-id>` 路径行为验证。
2. 构建与打包验证（在 `D:\AI\herdr`）：
   - 运行 `cargo test` 覆盖 `cli::machine` 与 `remote::attach` 相关测试；
   - 重新构建 release 版 `herdr.exe` 与全平台 `herdr-dist.zip`；
   - 回到 `D:\AI\sheprd` 执行 `build-portable.cmd --build` 刷新绿色版目录；
   - 在真实远端测试机执行 `herdr.exe machine upgrade <label-or-id>`，确认远端二进制替换成功、server 重启且已打开的 Sheprd GUI 自动恢复连接。
