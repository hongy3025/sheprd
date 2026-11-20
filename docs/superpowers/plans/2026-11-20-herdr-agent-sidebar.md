# Herdr Agent Sidebar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Sheprd 原生侧栏完整复刻 Herdr hongy-main 的 Agent 状态消息、Workspace 汇总、关联高亮及 OMP recap 阅读体验。

**Architecture:** 可选状态消息在连接工作线程与同 boot/revision 快照绑定，再沿现有事件与最新状态邮箱进入 UI。纯模型预备汇总和预览，现有 GPUI 行布局消费借用数据；窗口独立维护关联选择及 recap 阅读状态。CLI 与 hosted state 复用同一份事实，不另起 daemon 查询。

**Tech Stack:** Rust、serde/bincode、crossbeam 现有 worker、GPUI、TOML/toml_edit、Fluent；不增加依赖。

**Spec:** `docs/superpowers/specs/2026-11-20-herdr-agent-sidebar-design.md`

## Global Constraints

- 完整功能目标是 Herdr `hongy-main`；原版 Herdr 与 herdr-core 保持现有行为。
- 参考源码只读：`D:/AI/herdr/herdr`，本轮已核对的来源提交为 `eb898665`。
- 开发分支 `feat/hongy-agent-sidebar` 从 Sheprd `7574c78ce` 创建；不提交、不推送、不创建 PR。
- 保留冻结快照编码、连接代际隔离、连续排空的有序事件和合并最新状态邮箱。
- 消息全文最多 8,192 个 Unicode 字符，预览最多 200 个字符；空白折叠、截断带省略号。
- Workspace 优先级为 blocked > done > working > idle > unknown；idle/unknown 汇总不用旧消息。
- 原生 Rust 侧栏为完整呈现目标；Moonframe 只同步 hosted state，不重做 Luau 外观。
- render 不读取磁盘/网络、不扫描全量消息、不复制每行全文；动画使用 `crate::motion`。
- 每个 Rust/Python/shell/Swift 文件最多 1,000 行；测试放独立 topic 文件。
- 所有 Cargo 验证使用 `--locked`；短本地检查优先，长构建/全量/跨平台检查交给 CI。
- 以下命令均为未来实施时的验证指令，不代表已运行或通过；不无故重复成功检查。
- GPUI 新增测试函数统一使用 `agent_sidebar_` 前缀，供窄 filter 运行；
  下文测试行为名省略此前缀。代码片段给出关键断言，夹具复用所在测试模块的已有构造。

## Review Focus

1. 同一 revision 重复发送但内容不同，不能在半个 UI 更新中混合两份消息；Task 1 的重复版本测试固定规则。
2. JSON 字符转义使 200 字符预览膨胀，CLI 仍须遵守 256 KiB 总响应上限；Task 5 验证真实编码字节数。
3. 菜单、拖拽或折叠箭头吞掉的点击，不得改变关联目标；Task 4 覆盖被拒绝/非行点击。
4. 读取 recap 期间断线并复用同名 pane，内容和关闭后的焦点不得绑定到新连接；Task 4 验证代际与焦点。
5. 保存开关时磁盘文件被外部编辑，不能覆盖外部 token 行或 per-agent overrides；Task 3 验证冲突与 reload。

## 文件与职责

已有文件只追加窄入口；下列新路径是计划创建的模块，不是假定已有文件。

| 单元 | 文件 |
| --- | --- |
| 可选 wire 与消息类型 | 修改 `crates/sheprd-protocol/src/endpoint.rs`、`NOTICE.md`；新增 `crates/sheprd-protocol/tests/agent_status_messages.rs` |
| 连接内消息归约与 UI 缓存 | 新增 `crates/sheprd-client/src/session/agent_status.rs`、`session/agent_status/tests.rs`、`src/tests/agent_status_messages.rs`；修改 `event.rs`、`session.rs`；`ClientEvent::AgentStatusMessages(Arc<EndpointAgentStatusMessages>)` 按序发布；GPUI 的 active/pending 投影在匹配快照提交时切换 |
| 预备侧栏模型 | 新增 `crates/sheprd-gpui/src/sidebar/summary.rs`、`summary/tests.rs`、`state/agent_sidebar.rs`；修改 `sidebar.rs`、`state.rs`、`state/projection.rs` |
| 原生行和尺寸 | 修改 `sidebar/cell.rs`、`render/agent_rows.rs`、`row.rs`、`layouts/{herdr,minimal,orca,superset,parts}.rs`；新增 `sidebar/row/agent_sidebar.rs`、`sidebar/render/workspace_rows.rs` |
| 配置与持久化 | 修改 `config/sidebar.rs`、`herdr_settings.rs`、`herdr_settings/sidebar.rs`、`settings_window/controls/plugins.rs`；新增对应目录下 `tests/agent_sidebar.rs` topic 文件 |
| 关联与详情 | 新增 `sidebar/selection.rs`、`selection/tests.rs`、`sidebar/recap.rs`、`recap/tests.rs`；修改 `navigation.rs`、`window.rs`、`window/construct.rs`、`window/host_regions.rs`、`window/input.rs` |
| 控制协议与 CLI | 新增 `crates/sheprd-control-protocol/src/protocol/app/agent_sidebar.rs`；修改 `protocol.rs`、`protocol/app.rs`、`protocol/settings.rs`；GPUI 新增 `control/{client,server}/app/agent_sidebar.rs`，修改 app 分派、settings 保存及 CLI parsing/help |
| hosted state | 修改 `sidebar/hosted.rs`、`sidebar/hosted/devices.rs`、`sidebar/hosted/actions.rs` |
| 验证与说明 | 新增 `sidebar/layout_tests/agent_sidebar.rs`、相关 CLI/control topic 测试；修改 `smoke_sidebar.rs` 或拆出其 sidebar topic、`tests/live_gui.rs`、`locales/{en-US,zh-Hans}` 下现有相关 FTL、GUI README、`docs/cli.md`、`skills/herdr-gpui-browser/SKILL.md` |

`sidebar/render.rs` 当前 977 行，先把 Workspace 行构造提取到 `render/workspace_rows.rs`；
保留循环、行顺序、拖拽计数和位置追踪在原处。`session.rs` 当前 910 行、`state.rs`
948 行，新增行为分别放上表新模块，避免借机做无关重构。

## Task 1: 可选状态消息随快照原子发布

**Interfaces**

- 协议常量：`AGENT_STATUS_MESSAGES_CAPABILITY = "agent_status_messages"`；
  `AGENT_STATUS_MESSAGES_KIND = "endpoint.agent-status-messages.v1"`。
- `endpoint::EndpointAgentStatusMessages { boot_id: String, revision: u64, messages: BTreeMap<String, String> }`。
- 冻结的 `ClientShellSnapshot` 保持不变；不增加序列化字段，也不复制任何状态消息。
- worker 新增 `ClientEvent::AgentStatusMessages(Arc<EndpointAgentStatusMessages>)`。
  UI 将精确快照对应 projection 作为 active，较新 projection 暂存在 pending；
  只有 `commit_snapshot` 才替换 active，surface 等待期间继续显示旧的一致快照/消息。
- 私有 `session::agent_status::Projection::receive(&mut self, data: &str)`；
  `take_for(&mut self, boot: &str, revision: u64) -> Option<EndpointAgentStatusMessages>`。
  `Session` 每条连接持有一个 `Projection`，仅在 welcome 广告 capability 时启用；
  归约不消费未来 revision 或错 boot。

- [x] **1. 添加协议类型和归约失败测试。**
  协议测试用 hard-coded JSON 检查独立 companion 的必需字段以及 `BTreeMap` pane 映射；
  并固定 kind/capability 字面值；以现有 compatibility fixture 检验快照兼容，不重写 golden。
  新归约测试命名及关键断言：

  ```rust
  // message(boot, revision, text) 是该 topic 的合成 JSON helper。
  projection.receive(&message("boot", 2, "Run\n tests"));
  assert!(projection.take_for("boot", 1).is_none());
  assert_eq!(projection.take_for("boot", 2).unwrap().messages["p1"], "Run tests");
  assert!(projection.take_for("boot", 3).is_none());
  ```

  再覆盖 `stale_revision_and_wrong_boot_never_match`、
  `complete_projection_drops_panes_missing_from_the_new_revision`、
  `duplicate_revision_first_valid_projection_wins`、`malformed_optional_control_does_not_disconnect`、
  `unicode_full_text_is_capped_at_8192`；8192 正好保留，8193 截断为 8191 字符加 `…`。
- [x] **2. 用窄测试运行一次 RED。**
  `cargo test --locked -p sheprd-protocol --test agent_status_messages`；
  `cargo test --locked -p sheprd-client agent_status`。
  预期仅因新接口未实现失败，不能把环境/编译器错误当作行为 RED。
- [x] **3. 实现独立 wire 类型和有界 Projection。**
  只保留一个未来 companion；同 boot 同 revision 重复时首份有效数据胜出，
  更旧消息不覆盖。非法 JSON 丢弃匹配数据；下一快照绝不收到旧文本。
  projection 只通过 `ClientEvent::AgentStatusMessages` 有序事件发给 UI；
  来源保证 companion 先于快照。重复版本的首个有效 projection 在消费后也不可覆盖。
  归约对现有帧大小进一步约束至一个待处理 map，只按 boot/revision 精确匹配快照。
  行模型只查找实际存在的 pane，不渲染对应快照中不存在的消息项。
  使用安全单遍 UTF-8 规范化，丢弃空消息。有效数据移动进 Arc，不额外复制全文。
- [x] **4. 在 `Session::handle_message` 处理中发出独立可选事件。**
  未广告 capability 的 companion 继续忽略；每次新 Session 自动清空归约状态。
  保留现有 `ClientEvent::Snapshot` 行为和 `ClientShellSnapshot` 构造/
  serde/bincode 兼容，不改 surface 匹配、事件 queue 或 mailbox 契约。
- [x] **5. 添加并运行 worker 消费行为测试。**
  `crates/sheprd-client/src/tests/agent_status_messages.rs` 用现有 mock peer 发 welcome→companion→snapshot，
  检查 projection event 先于原样 snapshot；覆盖无 capability、空 map 清除、
  错 boot、未来 revision、断开后新连接和 snapshot/surface 分离。
  运行上述 Task 1 窄命令及 `cargo test --locked -p sheprd-protocol --test compatibility`。
  预期新增用例与原有 wire compatibility 均 PASS。

## Task 2: 缓存 Workspace 汇总和 Agent 预览

**Interfaces**

- 新建 `sidebar::summary::Prepared::new(
    snapshot: Arc<ClientShellSnapshot>,
    messages: Option<Arc<EndpointAgentStatusMessages>>
  ) -> Self`，
  以及 `workspace(&self, id: &str) -> Option<&WorkspaceSummary>`、
  `agent(&self, pane: &str) -> Option<&AgentPreview>`、
  `preview(&self, pane: &str) -> Option<(&str, bool)>`、
  `full_message(&self, pane: &str) -> Option<&str>`。
- `WorkspaceSummary { workspace_index: usize, tab_count: usize, pane_dots: Vec<PaneDot>, winner: Option<Winner>, other_tabs_attention: Option<AgentStatus> }`。
  `PaneDot { pane_index: usize, status: Option<AgentStatus>, winner: bool }`；
  `Winner { agent_index: usize, status: AgentStatus }`；
  `AgentPreview { agent_index: usize, preview: Option<MessagePreview>, idle_recap: bool }`。
  `MessagePreview { end: usize, truncated: bool }`；preview 返回 projection 中的 borrowed slice。
- `Prepared` 持有快照及对应 projection 的 `Arc`，workspace/agent 查询索引使用排序索引
  向量与 binary search；行/点/summary 只保存 `usize` 索引，不克隆 pane/workspace ID。
- `AgentSidebarState` 提供 `connected(&EndpointServerWelcome)`,
  `receive(Arc<EndpointAgentStatusMessages>)`,
  `commit_snapshot(Arc<ClientShellSnapshot>)`，`disconnect()`，`prepared()`，
  把 connection capability、current/pending projection 与 Prepared 作为一个状态单元。
- `LiveState` 新增 connection-local `AgentSidebarState`，容纳支持能力、当前与
  等待的 projection 及 `Option<Arc<Prepared>>`。`ClientEvent::AgentStatusMessages`
  只填 pending，不使可见数据变脏；仅匹配快照的 `commit_snapshot` 同时交换 active
  projection 和 Prepared。待 surface 的 snapshot 保留旧的一致展示；断线清空。

- [x] **1. 添加 `summary/tests.rs` 纯行为测试。**
  `winner_priority_precedes_focus_active_tab_and_stable_pane_order` 验证完整 winner 序；
  `pane_id_breaks_ties_when_no_status_pane_has_an_order` 验证稳定最终决胜；
  `workspace_counts_only_its_tabs_and_panes_and_marks_other_tab_attention`
  覆盖多 tab、多 Workspace、shell pane 与注意状态；
  `only_the_winning_agents_message_is_used_and_idle_recap_is_identified`
  验证获胜者只取自己的消息与 OMP idle recap；
  `message_preview_caps_unicode_without_copying_the_full_message` 验证 200 字符切片和完整消息。
  在测试内重排 agents 数组，结果保持基于 pane 顺序而非偶然数组顺序。
- [x] **2. 运行 RED：** `cargo test --locked -p sheprd-gpui --lib sidebar::summary`。
  首次编译在缺少 `Prepared`/`AgentSidebarState` 接口处进入预期 RED 阶段。
- [x] **3. 实现上述模型。**
  使用现有 `status_priority` 与 Agent 名称/标签解析，不定义镜像状态 enum。
  一次建立 pane/Agent 索引；Workspace 只聚合自身所有 tab，tab 数量来自 tabs。
  winner 保留身份，文本通过同 pane 的预览读取；idle/unknown 汇总使用现有本地化标签。
  无附件返回未启用状态，不改变原版/core 的模型或优先级。
- [x] **4. 接入 LiveState 提交与断开清理。**
  新 `state/tests/agent_sidebar.rs` 验证 pending metadata 不改变已显示汇总，
  匹配 surface 提交后一起变化；无 surface 的首快照正常提交；断开后 Prepared 为空。
  `cargo test --locked -p sheprd-gpui --no-default-features --lib sidebar::summary`
  与 `cargo test --locked -p sheprd-gpui --no-default-features --lib state::tests::agent_sidebar`
  均通过（5 与 4 tests）。

## Task 3: 现有原生布局呈现汇总、消息与开关

**Interfaces**

- `WorkspaceRow<'a>` 增加 `summary: Option<&'a WorkspaceSummary>`、
  `summary_text: Option<&'a str>`；`AgentRow<'a>` 增加
  `message: Option<&'a str>`、`idle_recap: bool`。`None` 保持原行路径。
- `SpaceLayout` 增加 `pane_strip: bool`、`status_text: bool`，均默认 true；
  默认 token rows 不全局重排，只有启用的 fork 默认展示使用新行排列。
- 新窄 enum `config::sidebar::SpaceFlag { PaneStrip, StatusText }`；
  `Edit::SidebarFlag { flag: SpaceFlag, shown: bool }` 保存到 daemon
  `[ui.sidebar.spaces]` 对应键，沿现有 `Settings::save` 后台原子保存与冲突检查。

- [x] **1. 添加配置和布局失败测试。**
  `config/sidebar/tests/agent_sidebar.rs` 验证默认值和独立关闭，不改变 rows/row_gap；
  `herdr_settings/tests/agent_sidebar.rs` 验证保留注释、styled rows、
  rows_by_agent，遇外部编辑返回现有冲突错误，reload 后才保存。
  新 `sidebar/layout_tests/agent_sidebar.rs` 验证上下文/名称/消息分行、
  Workspace 所有 pane 和 tab 徽标、无 Agent 不占空行、开关分别生效。
- [x] **2. 运行 RED：** `cargo test --locked -p sheprd-gpui --no-default-features --lib agent_sidebar`。
  首轮发现两处测试 harness 错误并修正；随后只剩预期的 SpaceFlag、
  SpaceLayout 字段与 SidebarFlag variant 缺失。
- [x] **3. 按职责抽出 Workspace 行构造，扩展共享行输入。**
  新 `render/workspace_rows.rs` 接收现有 Cell 所需的行数据，返回构造后的行元素；
  不移动外层的排序、计数、拖拽或 sticky 状态。所有其他 WorkspaceRow/AgentRow
  构造点显式给出 `None`，仅 fork 行绑定 Prepared。
- [x] **4. 在共享 `row/agent_sidebar.rs` 及各布局接入新内容。**
  原生默认展示为目录+Git计数+tab徽标+pane点、分支、获胜者状态；
  Agent 保留上下文后显示强调色名称和正文色消息，缺消息回退状态标签。
  自定义 token 行原样保留，新增摘要在其后，不重复插入已有 token。
  沿用 `layouts/parts.rs::Line` 的明确宽度预算；
  给目录和徽标优先预算，pane 点超出时用 `+N`，极窄宽度不越界、不负宽。
  rail 不增加多行；Devices 等没有 Workspace 行的布局不伪造 Workspace，
  但其 Agent 行仍消费同一消息输入。保持 notes/ports/PR badges、折叠和树干几何。
- [x] **5. 将两项设置接入现有 Plugins/侧栏值开关与 worker 保存。**
  用同一 `Edit::SidebarFlag` 解析并保存，禁用保存中的重复提交，
  watcher 重新加载后刷新准备状态；不要写 GUI local 配置或重置 daemon token rows。
  增加 en-US/zh-Hans 文案，其他 locale 按既有 fallback。
- [ ] **6. 运行窄布局/保存测试，完成合成原生烟测场景。**
  `cargo test --locked -p sheprd-gpui agent_sidebar`；
  将场景纳入现有 native sidebar fixture，覆盖每种原生布局、窄窗口、
  长中英文、深浅主题、100 个 pane 的溢出计数，以及混合 fork/原版/core 主机。
  `just test-sidebar` 在有桌面、编译成本可控时运行；否则明确留给 native CI。

## Task 4: 关联高亮、真实焦点与 recap 阅读

**Interfaces**

- `sidebar::selection::Target { endpoint: String, generation: u64, boot: String, workspace: String }`。
  `Selection::select(&mut self, target: Target)` 幂等；
  `matches(&self, endpoint: &str, generation: u64, boot: &str, workspace: &str) -> bool`。
  `Selection::retain(&mut self, valid: impl FnOnce(&Target) -> bool)` 清理失效来源。
- `SheprdWindow` 新增 `sidebar_selection: Selection` 和 `agent_recap: Option<Recap>`；
  初始化保持字段排序，新增逻辑放新模块。
- `Recap` 固定保存 endpoint/generation/boot/pane、点击时全文的 `SharedString`、
  FocusHandle、ScrollHandle 及现有 motion::Toggle。
  `open_agent_recap(&mut self, endpoint: &str, pane: &str, window: &mut Window, cx: &mut Context<Self>) -> bool`；
  `close_agent_recap(&mut self, window: &mut Window, cx: &mut Context<Self>)`；
  `render_agent_recap(&self, cx: &mut Context<Self>) -> Option<AnyElement>`。
  false 表示来源不支持、没有 idle recap 或已被其他 modal 阻止，CLI 另行分类错误。

- [x] **1. 添加 selection 和 recap 失败测试。**
  重复 select 同一目标不清除；同名跨 endpoint/boot/generation 不匹配；
  删除 Workspace、重连清空；无 capability 不启用。
  headless 场景覆盖点击 Workspace→所有相关 Agent 同背景、
  Agent 点击迁移关联、实际焦点独有成功色 marker。
- [ ] **2. 运行 RED：** `cargo test --locked -p sheprd-gpui agent_sidebar`。
- [x] **3. 接入被接受的行点击。**
  新关联 setter 只在 Workspace 行点击及 `open_agent_row` 成功路径调用；
  不能改变既有 Navigate/Rescroll 决策或把队列成功当 daemon 确认。
  增加 RowState 的独立关联标志，选中/关联使用一致背景，真正焦点独立绘制；
  不挪用原本表示菜单悬停的 highlighted 字段。
  在窗口读取连接状态的既有更新入口清理来源失效的 Selection。
  搜索结果、嵌套、设备树及 hosted 行动作调用相同 helper。
- [x] **4. 实现 recap 弹层、键盘和输入隔离。**
  文本点击 stop_propagation，正文可滚动，明确关闭按钮；
  回车/空格可激活详情，Escape 关闭，开关均使用 motion；
  UI 仅在打开时复制一份有界全文，后续消息不变更正在阅读的文本。
  接入 `append_overlays` 和窗口键盘/鼠标/IME/paste 路由，
  使用独立 FocusHandle，不得有任何终端 input/scroll 请求漏下去。
  连接失效时保留只读文本但标记来源已离线，关闭时不恢复到同名新 pane。
- [ ] **5. 用真实交互场景固定边界。**
  `recap_click_does_not_navigate_or_send_keys`；
  `fold_drag_menu_refusal_does_not_relink`；
  `recap_survives_disconnect_without_rebinding`；
  `escape_and_paste_stay_in_recap`。
  在原生 sidebar fixture 中做键盘与滚轮检查；
  运行 `cargo test --locked -p sheprd-gpui agent_sidebar`，原生检查纳入 `just test-sidebar`。
  此任务不改 recent_panes、导航 admission、surface fences、modifier release；
  如确需改变其中任一项，先加载 recent-pane-order skill 再验证对应不变量。

## Task 5: CLI、hosted state 与端到端交付

**Interfaces**

- `UiOp::AgentSidebar`、`UiOp::AgentRecap { pane: String }`；
  `app::Reply::AgentSidebar { state: AgentSidebarState }`。
- `AgentSidebarState { selected: Option<SidebarSelection>, hosts: Vec<SidebarHost> }`：
  每 host 有 id、supported、workspaces、agents；Workspace 含 id/tab_count/
  pane_dots/winner/other_tabs_attention；Agent 含 pane/workspace/status/preview/
  idle_recap/focused/linked。控制协议借用既有 AgentStatus，不新增镜像 enum。
- CLI：`sheprd ui agent-sidebar`；`sheprd ui agent-recap --pane ID`。
  无调用者时使用所选 host；有调用者时沿既有 caller/daemon/socket 定位，
  找不到不跨 host 猜测。只读输出不返回每 Agent 的 8,192 字符全文。
- 现有 `config set` 只改 GUI local 文件，不能直接保存这两个 daemon 键；
  因此按设计的条件补齐 `sheprd ui agent-sidebar-config
  [--pane-strip on|off] [--status-text on|off]`。
  `SettingsOp::AgentSidebar { pane_strip: Option<bool>, status_text: Option<bool> }`，
  对应 `settings::Reply::AgentSidebar { pane_strip: bool, status_text: bool }`；
  无参数只读，有参数走 Task 3 的同一后台 editor。
- 扩展 `sidebar::hosted` 的 Agent message/idle_recap/linked 和 Workspace summary，
  stamp 纳入关联选择；hosted recap 点击沿原生同一 open helper。

- [x] **1. 添加协议、parser 和 handler 失败测试。**
  分别在 protocol/app 与 settings 的 tests topic、GPUI CLI tests、
  `control/server/app/tests/agent_sidebar.rs`、client app tests 添加用例。
  覆盖 --json/--window、非 UTF-8 外层 OS 参数行为不退化、
  pane 缺失/空值/超界、无消息退出 4、找不到退出 5、modal 冲突退出 6、
  不支持退出 8。只读没有 fork host 时返回明确 Unsupported；
  混合窗口各 host 标记 supported，不替原版或 core 伪造摘要。
- [ ] **2. 运行 RED：**
  `cargo test --locked -p sheprd-control-protocol agent_sidebar`；
  `cargo test --locked -p sheprd-gpui agent_sidebar`。
- [x] **3. 实现协议、CLI/help/output 和 server 定位/派发。**
  新 types 放 `protocol/app/agent_sidebar.rs` 并从 app re-export；
  本次 app control shape 变更将 `PROTOCOL_VERSION` 从当前 13 升到 14，
  若其他工作已升级，则基于实际新版本再递增，不覆盖其他工作。
  handler 只读 Task 2 的缓存；详情调用 Task 4，开关调用 Task 3，
  不在 UI/control 中扫描磁盘或另查 daemon。
  最大控制消息为 `MAX_MESSAGE = 256 * 1024`；
  使用实际 JSON 编码预算，溢出返回明确失败，不静默少报主机或 Agent。
  明确限制新请求 pane ID 长度为 512 字节，并在 parser/handler 双端验证。
- [x] **4. 接入 hosted state 与文档。**
  测试关联改变、消息改变和断线会使 hosted stamp 失效；
  更新 GUI README 的功能、兼容边界及两个 daemon 开关，
  `docs/cli.md` 三条命令和退出码、托管 skill 命令表；
  保持 skill name，若改 skill 内容先加载相关 skill 编辑规则。
- [ ] **5. 验证 JSON 膨胀和端到端身份隔离。**
  `escaped_previews_respect_control_message_limit` 用真实编码的引号、反斜杠、
  CJK 文本和大量 Agent 检查每个响应不超过 MAX_MESSAGE；
  `recap_caller_never_targets_another_host` 在同名 pane 的两个 host 上验证。
  完成以上 Task 5 窄命令；检查生成的 CLI --help 与合成窗口的 --json 输出，
  不把纯 parser/headless 测试描述成原生端到端验证。

## 最终验证与交付

- [ ] 仅运行尚未覆盖的相关短检查：`just check-file-size`、`just check-sorted`、
  `just check-i18n`、`git diff --check`；格式检查按修改的 Rust 文件定向运行，
  不无故重跑全量测试。若命令包含编译且预计超过本地短检查预算，记录留给 CI。
- [ ] 有桌面时完成合成数据原生截图和鼠标/键盘/滚动检查；
  验证 hongy-main 真实数据链路时只运行沙箱子进程，显式指定
  `HERDR_TEST_BINARY=D:/AI/herdr/herdr/target/debug/herdr.exe`，
  先确认文件存在与版本，不自动构建/改动参考 Herdr 或连接个人 daemon。
- [ ] 原生真实 daemon 检查复用现有 `just test-gui <binary>`；
  较长编译、release 性能、default/all-features 和其他平台由对应 CI 承担。
  headless、native synthetic、live daemon 三种结果分别报告。
- [ ] 清除临时脚本；交付完整功能、确切验证结果、未运行的原生/平台检查。
  用户未请求时不 commit/push/PR；日后获授权提交时只用 eval 的 commit namespace，
  conventional subject 描述用户可见结果。

## 执行方式

推荐本会话 Native 执行：Tasks 1→2→3→4→5 共享快照与行输入接口，
顺序实现能避免多 Agent 同时修改热区。每个任务实施前加载所需执行/TDD skill，
先添加有消费者意义的失败测试，完成后统一跑该任务最窄验证。

另一选择是 subagent-driven：仅在符合仓库委派规则、有至少两个真正独立的大切片时
并行委派；不为单个小任务启动 Agent，也不逐任务串行外包。
无论选择哪种方式，最终都做一次独立整体审查；在实际实施前确认是否原地开发
或经用户许可建立 isolated worktree。当前只已创建开发分支，未创建 worktree。
