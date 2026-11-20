# Herdr hongy-main Agent 侧栏体验迁移

状态：用户已于 2026-11-20 批准。方案 1：融入现有原生侧栏。
产品代码实施前须完成并确认实施计划。

## 意图与范围

在 Sheprd 的原生侧栏中复刻 Herdr fork 的 Agent 体验优化，让用户在
Workspace 与 Agents 两个视角看到工作意图、完成结果、待处理状态以及相互关联。

用户已确认：

- 完整功能目标是 `D:/AI/herdr/herdr` 的 `hongy-main` daemon。
- 原版 Herdr 与 herdr-core 保持现有兼容，不要求补齐相同功能。
- 选择融入现有原生侧栏，不新增专用布局。
- 在本仓库创建新的开发分支实施；Herdr 仓库只读。

不迁移远程 `herdr-dist.zip` 安装、服务端 Agent 上报逻辑或 OMP 插件。
不接管、升级、停止或迁移用户的 daemon。

## 已核对的来源与当前实现

来源基线为 Herdr `master` 与 `hongy-main` 的共同点
`ee5bfb5041bb3e4a49d591c72308aae3c81ecdde`。
本次核对的来源分支末端为 `eb898665`。

相关变化包括 `d3458a59`、`9bca37a9`、`6273ee25`、`84ada7b0`、
`c81e0456` 与 `eb898665`；以末端代码的实际行为为准，不按提交逐个照搬。

Herdr 用独立的 `endpoint.agent-status-messages.v1` 控制消息传递
`boot_id`、`revision` 和 pane 到文本的映射，在对应快照之前发送。
消息不进入冻结的 `shell.snapshot.v1` 编码。欢迎消息声明
`agent_status_messages` capability。

Sheprd 目前：

- `sheprd-client/src/session.rs` 忽略不认识的可选控制消息。
- `sheprd-protocol/src/wire.rs` 的 `ClientShellAgent` 没有状态消息字段。
- 原生侧栏已有 Workspace/Agent 行模型、共享布局部件、状态优先级和
  Herdr 1 / herdr-core 的统一 Agent 列表。
- Agent 的现有 `status_text` 是状态标签，不是本次新增的工作消息。

## Approaches

1. **融入现有原生侧栏，用户已选择。**
   扩展现有数据链路与共享行部件，复用主题、密度、导航、菜单和设置。
   保持信息与交互语义一致，不追求终端字符的像素复刻。
2. **新增专用布局，未选择。**
   可更接近 Herdr 排版，且不改变其他布局，但增加长期维护分支，
   也使完整体验依赖切换布局。

不采用直接移植终端绘制代码：Ratatui 与 GPUI 的渲染、几何和输入模型不同。

## 数据接入与生命周期

- 在协议层定义可选状态消息类型与 kind/capability 常量，不修改冻结字段顺序。
- 在客户端连接工作线程接收和归约消息，不在 UI 线程解析传输或读取 daemon。
- 状态属于连接、boot、revision 和 pane 的组合，不能仅按 pane ID 缓存。
- 只把同 boot、同 revision 的消息与快照一起发布；未来消息暂存且有界，
  旧消息不得覆盖新状态。重连、detach、换 boot 后不继承旧消息。
- 映射是完整替换，不是增量补丁：没有 pane 条目的下一份对应映射清除旧文本。
- 启用 capability 的来源快照不得误用上一 revision 的消息；
  不支持此 capability 的连接继续走原路径。
- 复用现有有界帧、事件和最新状态邮箱；不引入无限队列、额外轮询或后台 API 请求。
- 工作文本属于不可信内容。规范化为空白折叠的可打印单行，最多 8,192
  个 Unicode 字符；预览最多 200 个字符，截断显示省略号，保留 UTF-8 边界。
- 无效可选消息不使终端连接失败；不允许它保留或套用旧状态。
  实施时确定一个有界的忽略/清除策略，并用行为测试固定。

## Workspace 汇总

展开的原生 Workspace 行增加以下信息，使用现有布局部件而不是新布局：

- 目录/Workspace 名与 Git 同步计数在主信息行，分支独立展示。
- tab 数量徽标，颜色取其他 tab 中最强的 Agent 状态。
- 覆盖本 Workspace 所有 tab 的 pane 状态点；非 Agent pane 使用中性标记。
  切换活动 tab/pane 不改变被统计的 pane 集合。
- 有 Agent 时增加代表 Agent 名称和状态摘要；没有 Agent 不占空行。
- 代表选择优先级：blocked > done > working > idle > unknown。
  同级依次按焦点、活动 tab、快照 pane 顺序、pane ID 稳定决胜。
- 只使用获胜 Agent 自己的文本，不能借用另一个 Agent 的消息。
- idle / unknown 的 Workspace 摘要显示状态标签，不显示过时消息；
  OMP idle recap 仍可在对应 Agent 行查看。
- 名称使用主题强调色，消息使用正文色；不硬编码暗色主题颜色。

汇总只涵盖当前 Workspace，不能把同仓库的其他 worktree 混入。
大量 pane 时保留完整模型，显示受行宽约束；使用可识别的溢出计数，
不能挤出目录、徽标或把状态点绘制到行边界之外。

## Agent 行与详情

- 保留上下文行，Agent 名称与工作消息分别成行。
- 有消息显示消息预览；无消息回退到现有本地化状态标签。
- 状态消息不替代真实状态、状态图标或 daemon 的活动判定。
- OMP idle recap 可点击打开原生可滚动文本详情，显示最多 8,192 字符的完整
  规范化消息。其他消息不额外增加来源中没有的点击操作。
- 详情使用现有弹层/详情呈现惯例；支持关闭、Escape 和键盘访问。
  打开/关闭不向终端发送输入、不切换 pane，也不改变 Workspace 关联。
- 详情打开时保留那次点击的消息与来源标识；后续消息不使阅读内容跳动。
  来源断开或消失时不把该详情重新绑定到同名新 pane。
- 名称使用主题强调色，消息使用正文色；焦点标记使用主题成功色。

## 关联选择与真实焦点

- 独立保存窗口级关联目标 `(endpoint, workspace)`，不修改 daemon 活动状态。
- 点击 Workspace 后，其行与所属 Agent 行使用一致的选中背景。
  再次点击同一 Workspace 不取消；点击其他 Workspace 替换关联。
- 点击 Agent 时把关联切换到它的 Workspace，继续执行既有 pane 导航。
- 焦点 Agent 另有绿色激活标记；关联高亮不意味着多个 pane 同时获得焦点。
- 关联只在对应支持功能的 Herdr 1 连接生效，不改变原版或 core 行行为。
- 端点身份参与全部比较；不同主机上同名 Workspace/pane 不能互相点亮。
- Workspace 删除或连接代际改变时清理关联；不自动导航到其他主机。
- 保留 Agents 排序、分组、嵌套、滚动、右键菜单及折叠操作。
  折叠箭头与摘要详情点击不冒充 Workspace/Agent 行点击。

## 原生适配与配置

以下是实现方提案，不是用户额外指定的要求：

- 扩展共享行输入和布局部件，让现有展开布局都能呈现新增事实，
  保持各布局自己的间距与装饰；折叠 rail 不强塞多行文本。
- 接入来源已有的 `[ui.sidebar.spaces] pane_strip` 和 `status_text`
  开关，缺省开启；保留自定义 token 行与 per-agent overrides，
  不把旧用户配置重置成新默认。
- 新开关通过现有设置编辑与 CLI 配置入口访问；若现有入口不能表达，
  在同一变更中补齐配置请求、解析、帮助与测试。
- 在已有 `sheprd ui` 命令组新增 `agent-sidebar` 只读命令，
  返回当前窗口准备好的关联目标、Workspace 汇总和 Agent 消息；
  新增 `agent-recap --pane ID` 打开与鼠标点击相同的原生详情。
  后者默认使用调用者所在主机，不能因 pane ID 重名选择其他主机。
  两个命令均支持 `--json`、`--window`、现有退出码和严格响应边界。
  这两个命令暴露 app 展示/持有的状态，不重复 Herdr 的 pane 管理。
- 可读命令输出无 capability 时明确不支持；不伪造消息。
- 新增文字使用现有 i18n 机制；新原生 UI 支持焦点与无障碍标签。
- 必要的显隐动画使用 `crate::motion`，不为消息刷新添加延迟动画。
- 原生 Rust 侧栏为本次完整呈现目标；可选 Moonframe shell 复用已有
  hosted state 的新增事实，不另行重做其 Luau 外观。

## 性能与边界

- 汇总在快照/状态变化时准备并复用；render 不重复全量扫描、规范化长文本。
- 不为每行重复复制完整消息；所有缓存受现有帧与快照规模约束。
- 不引入新 crate、runtime、传输通道或服务容器。
- 每个代码/测试文件保持在 1,000 行以内，按职责拆分。
- 不保存消息到磁盘，不将完整 Agent 消息写入日志或提交为测试素材。

## 验收与验证

1. 协议/客户端：真实 socket mock peer 覆盖消息先于快照、跨 revision、
   stale boot、重连、清除、无 capability、畸形可选消息和长 Unicode 文本。
   旧快照编码保持兼容。
2. 纯模型：优先级、稳定决胜、跨 tab 集合、非 Agent pane、
   其他 tab 徽标、Workspace/主机隔离、无消息回退。
3. 交互：重复点击持续高亮、点击另一 Workspace/Agent 迁移关联、
   真正焦点标记、删除与重连清理、详情不触发底层导航。
4. CLI：请求、解析、帮助、结构化输出、窗口/主机定位、
   不支持路径与响应边界。
5. 原生桌面：使用明确指定的测试 daemon 和隔离配置验证实际窗口；
   检查窄宽度、长中英文消息、大量 pane、滚动、键盘/Escape、
   深浅主题、折叠与既有布局。截图只使用合成测试数据。
6. 更新 GUI 文档、CLI 文档与适用的托管 skill；发布说明来自 conventional
   commit，不手写生成的 CHANGELOG。

本地先跑相关短检查；较长构建、全量测试和跨平台验证留给 CI。
不能运行的原生或平台检查必须明确标为未验证，不能用 headless 冒充。

## 交付步骤与实现方裁量

设计获批后先读取 writing-plans，生成实施计划，再从本仓库当前
`hongy-main` 创建 `feat/hongy-agent-sidebar` 开发分支。
保留用户与其他 Agent 的工作；本请求不授权提交、推送或创建 PR。

实现方决定模块拆分、具体 GPUI 元素、共享缓存位置和最窄验证命令，
但不能改变上面的行为、兼容边界和完整交付范围。
