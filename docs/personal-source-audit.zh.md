# Fury 独立 QA：源码与验证边界

上游基线 d0e5e3fa81b1b483b10ccd94b57630156ac3e527，2026-11-16 云端 Linux。源码审查不等于 Mac 实测。

## 已执行

`python3 /workspace/shared/fury-qa/sql_profile_isolation.py`：从当前 store.rs 提取完整 profile UPSERT，用临时 SQLite 创建 5 行并关闭/重新打开数据库；修改第三行代理、刻意传入不同 seed；验证另四行完全相同、第三行 seed/persona 保持、代理更新。5 项断言通过，临时数据库 12,288 字节。结果 `sql_profile_isolation.result.json`。这是 SQL 行为实测，不是 Rust Store、Cookie 持久化或浏览器端到端测试。

## 源码观察

- `store.rs::upsert_profile` 只在插入时生成 seed；ON CONFLICT 更新不触碰 seed。`ProfileDialog.tsx` 修改已有资料时保持 persona/seed，App.tsx 的代理切换也保留其他字段。主动 reseed 是独立操作，不应误点。
- `paths.rs::profile_dir` 用 profile ID 单独路径；Mac 默认数据根 `~/Library/Application Support/Fury`，FURY_HOME 可隔离整个安装。真实 Cookie/登录状态由各自 Chromium 目录保持，尚未启动目标内核验证。
- `ipc.rs` 默认拒绝无代理启动，只有明确 allow_no_proxy 才允许直接出口；空白环境可以先保存，但默认不能启动浏览器，直到填代理或明确允许直连。
- `launcher.rs` 只传本地 relay 地址，无上游用户名密码；无 loopback bypass，禁用 QUIC。`relay.rs::dial` HTTP/SOCKS 分支失败即返回错误，`handle` 返回 502，不尝试 Direct 分支。Direct 是主动选择的独立模式。
- HTTP CONNECT 使用 Basic 认证，SOCKS5 使用 RFC1929；两者的普通 HTTP/SOCKS 到代理这一跳本身不加密，不能将“密码落盘加密”理解为代理链路加密。HTTP CONNECT 网站 TLS 保护网站内容，但普通代理认证仍可能被链路观察者读取。
- SOCKS5 CONNECT 发送 ATYP=域名，HTTP CONNECT 发送目标域名，由上游解析目标。代理服务器本身若用域名仍需本机 DNS 解析；没有证明整个操作系统不发生其他 DNS。
- WebRTC 行为依赖 `core/patches/0070-webrtc-policy.patch` 及内核配置，不依赖旧无效开关。必须在实际内核上验证 STUN/ICE 和抓包，现阶段不能保证无泄漏。
- `ipc.rs` 时区选择为显式 profile timezone > 代理缓存/本次检查结果 > UTC，并记录 fallback 警告。代理检查服务不可用时不能宣称自动适配成功。切换代理可以改变时区/语言，但不重置 seed，这是按代理地区调整而非资料重建。
- 本地 `@local` 白名单可以允许特定 LAN 直连，这是显式例外，不应开启后仍宣称所有请求走代理。

## 占用/保留边界

- 源码 checkout 初测 114 MiB（含 Git）；实际构建后需重测。
- 上游 logging.rs 4 MiB 轮转只在启动发生；连续运行日志没有硬上限。已向实现负责人报告，不可称已解决。
- profiles 资料长期保留，删除使用软删除，可恢复；彻底清除是另一步，不应自动清除 Cookie/Storage。
- usage.rs 可以量测缓存/保留数据，trim 只在资料关闭时清除 bundle::SKIP_CACHES 白名单。Service Worker 与 Network Action Predictor 有意保留。此功能是手动清理，不是全盘配额或无限增长保护。
- 临时目录 Drop 清理不覆盖进程崩溃/机器断电，需要残留目录维护策略；构建 target、node_modules 与 Chromium 源码/产物和用户资料分离。

## 待验证

HTTP/SOCKS 本地认证与失败、五个 Rust Store 环境、launcher 参数、缓存清理可用 Rust 单元测试补充。Mac GUI 开关窗口、Cookie 与 LocalStorage 隔离、真实内核 DNS/WebRTC、真实时区、Keychain 均未实测。没有访问 Claude、没有真实代理凭据、没有外部反检测测试。

## 后续实际 Rust 验证（替代上面的待测状态）

工具链 `/tmp/fury-rust`，Linux 构建运行。新增 relay.rs 的 3 个 personal_* 防御测试：普通 HTTP Basic 认证成功、SOCKS5 用户名密码和远端域名报文正确、HTTP/SOCKS 上游关闭时不连接可达的本地目标。临时测试凭据仅存在测试代码/内存，无真实凭据。

- relay 36/36；launcher 12/12；usage 5/5；logging 3/3；bundle 13/13 均通过。
- 汇总 `cargo test -p fury-agent --locked -- --skip relay::tests::with_no_proxy_a_public_name_still_dials --skip relay::tests::with_no_proxy_a_public_name_with_a_private_answer_is_refused`：279 passed, 0 failed, 2 ignored, 2 filtered out，23.02 秒。完整日志 `agent-local-suite.log`。
- 两个 filtered 用例依赖公网 example.com DNS/TCP、sslip.io DNS，最终汇总排除；最初单独 relay 测试运行曾包含这些上游用例，发现后停止在汇总中使用它们。不能声称最初全部测试仅使用 loopback。没有访问 Claude，没有传送真实代理凭据。
- logging 已由保留策略实现者改为运行时有界日志，3 个测试覆盖轮转、超大事件和失败不增长，前述 startup-only 风险仅描述原版。
- target 实测 2.2 GiB；QA 目录约 100 KiB。浏览器资料目录尚无真实浏览器实例，不能用 fixture 占用估算实际每环境成本。
- 此汇总仍不是 Mac Keychain/GUI、实际 Chromium/真实 DNS-WebRTC验证。测试成功不证明真实代理或 Claude 接受登录。
