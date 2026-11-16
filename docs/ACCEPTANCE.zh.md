# 个人版交付验收（2026-11-16）

## 已完成、云端验证

- Linux x86_64（Debian 13、9 逻辑 CPU、9.7 GiB RAM、无 swap、初始可用约 57 GiB）。用 Node 24、npm 11、Rust 1.100；Fury 原版依赖锁定，不需要用户电脑。锁定前端依赖后 `tsc --noEmit && vite build` 通过；只有包体积提示。前端产物不是 Mac App。
- Rust Agent 聚合测试：**279 通过、0 失败、2 忽略、2 过滤**。忽略的两项要真浏览器（书签、内核 rekey）；过滤的两项含外部 DNS/TCP，由本轮最终聚合明确跳过。早期单独 relay 测试包含这两条上游外部网络测试，未接触 Claude，也未传出真实凭据。
- 五个空白独立环境：SQLite 实测创建和重开，五个 seed 不同且重开不变；改变第一条代理引用和固定时区后，其他四条未动。未打开五个真实 Chromium 进程。
- HTTP Basic、SOCKS5 用户名/密码：环回测试实测临时凭据送到对应的测试代理，SOCKS5 远端接收目标域名。故意让 HTTP/SOCKS5 上游失效，以仍可达的本地目标证明 relay 未回落直连。不是系统级网络审计。
- Vault：测试注入缺失密钥和加密失败，禁止新明文写入；已有密文、旧记录和成功加密后的读取仍正常。真实 macOS 钥匙串还未测试。
- 日志：运行中分段轮替、限流超大事件、轮替失败时拒绝写入；保存空间上限见 [保留策略](personal-retention.md)。云端实测构建占用约：Rust `target` 2.2 GiB、前端依赖 89 MiB、前端产物 2.4 MiB；固定内核归档 107,098,824 字节。没有 Mac 上五个真实资料及缓存的体积读数。
- 预编译 arm64 内核：计算 SHA-256 并与发布者清单及 API 比对；读取确认 Mach-O arm64；校验工具拒绝错长度和同长度错散列。**没有在云端运行该 Mac 内核**。见 [构建审查](BUILD-AUDIT.zh.md)。

复现 Agent 聚合测试（不访问 Claude）：
```sh
cargo test --locked -p fury-agent -- --skip with_no_proxy_a_public_name_still_dials --skip with_no_proxy_a_public_name_with_a_private_answer_is_refused
cd desktop && npm ci && npm run build
```

## 尚未验证 / 被环境阻塞

- 已准备 macOS arm64 构建脚本、固定版内核校验工具。没有授权的 Apple Silicon 云构建机和 Xcode；完整内核构建还需 Chromium 工具和约 200 GiB 空间。没有生成 .app / .dmg 或签名、公证产物，没有使用用户 Mac、没有调用 CI。
- 需要真正 Mac 上验证五个环境的启动/关闭/重开、独立 Cookie 和网站状态、网页实际时区、系统钥匙串可用/不可用、浏览器和系统层 DNS/WebRTC、代理改动后的流量与实际磁盘增长。
- 验收应用应使用回环测试服务，不用真实 Claude 账号或代理密码；用没配置代理、有效代理、认证失败、关闭上游四种状态对照；修改环境 1 后重开，检查其本地站点 Cookie 留存及环境 2–5 未变，然后再检查浏览器网络观测是否符合预期。
