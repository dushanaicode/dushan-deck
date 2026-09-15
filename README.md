# Dushan Deck

本地优先的 AI 模型管理与桌面工作台。Rust + Tauri 2 + React / TypeScript + Vite + Tailwind CSS + SQLite。

## 当前能力

- 主窗口、悬浮窗、托盘、单实例与快捷键；统一退出。
- Claude、OpenAI、xAI、Z.ai、Zhipu、Kimi Code、DeepSeek、Antigravity、Cursor、Cursor Agent 专区。
- API Key、Claude Code JSON、Codex JSON 和单个 Quota 账号 JSON 导入。
- 独立账号与完整凭据组加密保存；相同凭据重复导入更新原记录。
- 模型连接配置、专区收藏、浮窗偏好与任务历史持久化。
- 本地数据库检查与中断任务恢复。
- 原版 Quota 悬浮窗：默认 290×430、5 套主题、图片背景、模糊、透明度、置顶、圆角、卡片排序、倒计时和用量筛选。
- Rust 额度查询、OAuth 凭据续期、每分钟后台刷新与错误退避；官方额度、远端用量和本机日志分别展示。

当前处于开发阶段。OAuth 浏览器登录、客户端写入、检测实验室、宠物、computer use 和真实数据迁移尚未完成。查询成功不自动合并账号，也不代表模型调用能力已验证。

## 接入额度与用量

1. 在设置中收藏需要的专区，创建并解锁本地凭据库，再导入账号。默认收藏 Claude / OpenAI。
2. Claude / OpenAI 订阅额度需要对应 OAuth 登录态；普通 API Key 不等于订阅授权。Cursor IDE、xAI、Antigravity 使用 Quota 账号 JSON，其他来源按表单提供的方式接入。
3. Quota JSON 支持单个导出记录或单元素数组；原字段和完整凭据组加密保留。Antigravity 沿用原版客户端注册；自定义 Google OAuth 登录需在同一记录中成组提供 `client_id` 和 `client_secret`。
   OAuth 会话会在后台续期；客户端自动回写尚未接通，共用会话的原客户端可能需要同步更新凭据。
4. 本机用量在设置中添加来源：Codex、Claude Code、OpenCode、OMP、Kimi Code CLI、Grok CLI。OpenCode 选择数据库绝对路径，其余选择会话目录，并明确账号归属起点；切号时添加新的起点，保留旧起点。
5. 打开悬浮窗，在设置中启用“用量信息”，选择时间范围与客户端。无法确认归属的记录会提示，来源文件只读。

远端用量覆盖 OpenAI 账号活动、Anthropic Admin Usage（需相应权限）及 Z.ai / Zhipu 模型用量。网络或限流失败保留可用旧数据。当前验证采用合成协议、隔离数据库和 Windows 实机操作；尚未使用真实账号验证各私有接口，也没有 Mac 实机证据。

单轮本机扫描限制为 5 秒、10,000 个文件、合计 512MB（单文件 128MB），超出会提示并保留可用旧用量。可选择更具体的目录；不会静默当成零用量。

## 开发

在项目根目录运行，需要 Node.js 24、Rust 1.98，以及目标平台的 Tauri 2 编译环境。当前验证平台为 Windows x64；macOS / Linux 尚无实机证据。

```powershell
node scripts/deck.mjs install
node scripts/deck.mjs dev
```

开发入口自动将临时文件、依赖缓存、前端依赖、Rust target、前端构建输出、图标、Tauri schema 和应用状态放入项目 `Temp/`。不修改 HOME / USERPROFILE。不要直接运行未隔离的 npm 安装、cargo 或 Tauri 命令。

`node_modules`、`src-tauri/gen` 只是指向 `Temp/` 的目录链接。`package-lock.json` 和 `Cargo.lock` 固定依赖；不运行依赖安装脚本。

```powershell
node scripts/deck.mjs build     # 调试可执行文件，无安装包
node scripts/deck.mjs check     # TS、格式、Rust fmt / clippy
node scripts/deck.mjs test      # 核心行为与浏览器边界验证
```

Windows 原生验证：先 `. ./scripts/environment.ps1`，再运行 `node scripts/native-smoke.mjs`；使用合成凭据与独立 Temp 状态，报告和截图保存在 `Temp/verification/native-*`。

原版视觉对照：保留 `Temp/references/dushan-quota`，启动 `node scripts/deck.mjs web` 后运行 `node scripts/compare-quota-float.mjs`。对照图和像素差异在 `Temp/quota-float-20260915/visual/`。普通浏览器交互测试只在测试环境模拟 IPC。

编译产物：`Temp/build/rust/debug/dushan-deck.exe`。开发数据：`Temp/dev-state`。直接启动时必须显式提供 `--state-root <绝对路径>`；不推断用户数据目录。

隔离验证可追加 `--offline` 禁止后台服务请求；正常启动在解锁后自动刷新。悬浮窗样式与素材来源见 [第三方许可](THIRD_PARTY_NOTICES.md)。

## 本地凭据库

当前跨平台加密原型采用 Argon2id + AES-256-GCM。口令只在内存中使用，每次重启重新解锁；库内保存随机 salt、校验密文与完整凭据组密文。账号、来源类别、模型连接等元数据不加密。口令丢失无法解密，系统安全存储集成留待平台原型验证。

不会因同一来源、邮箱或 Key 尾号合并账号。未通过服务端验证的登录会话保留独立账号记录；外部轮换后重新导入的会话可能形成新待确认记录，可靠身份合并属于后续完整链路。

主窗口“×”收起到托盘；浮窗“×”仅隐藏自己；托盘或窗口中的“退出应用”等待本地工作结束。`Ctrl / Cmd + Shift + D` 打开主窗口。

## 结构

```text
src/                  按 feature 组织的界面和集中 IPC 契约
src-tauri/            桌面装配、commands、窗口与退出
crates/deck-core/     账号、连接、存储、设置与任务核心
tests/                页面和平台验证代码
scripts/              隔离开发与验证入口
```

所有外部服务、真实凭据迁移、安装升级与发布分别验证和授权。本项目没有启动本地 HTTP 业务后端。

## Git 管理

- `main` 保存已验证的项目基线。
- 每项独立修改使用 `feat/...` 或 `fix/...` 分支，以小步提交保留变更记录。
- 合并前完成与改动相关的验证；临时文件、凭据、依赖和构建产物不纳入版本库。
