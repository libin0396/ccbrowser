# Fury 个人版构建审查（2026-11-16）

## 已实测
- 云环境 Linux x86_64，9 个逻辑 CPU，9.7 GiB RAM，无 swap；检查时磁盘剩余 57 GiB。
- git 2.52.0、Node 24.22.1、npm 11.9.0、GCC、Python 可用；初检无 Rust/Cargo、Clang、CMake、Ninja、Xcode。
- GitHub 页面、API、官方 release 下载可访问。不是对所有依赖站点的完整网络验收。
- 环境目录仅提供本云机和用户 MacBook，没有授权云 macOS 构建机；未连接或使用用户 Mac。
- 上游源码 d0e5e3fa81b1b483b10ccd94b57630156ac3e527；workspace 0.5.7，Chromium pin 157.0.8103.39。Rust 声明最低 1.80，不代表当前锁定依赖在 1.80 可编译。
- build-personal-macos-arm64.sh 经 bash -n，通过；在 Linux 执行会在任何构建动作前明确 exit 2。macOS 构建分支未运行。

## 浏览器内核来源核验
本目录中的 fury-core-0.5.7-macos-arm64.tar.xz 是上游预编译产物，不是本次编译。
来源 https://github.com/furyteamtop/fury-antidetect-browser/releases/download/v0.5.7/fury-core-0.5.7-macos-arm64.tar.xz

- 实际下载 107098824 字节。
- 实际 SHA256：148dbb1560f7b7e4ed9e0ffc34734597a197c2df3e3c54f3c4ddab35ee1d299a。
- 与 GitHub release API digest 以及该 release 的 SHA256SUMS 两者一致。
- tar 只读检查 1368 个条目，常规文件总大小 347770173 字节；主程序 Mach-O CPU type 0x0100000c（arm64）。
- 没有解包安装或执行内核。未独立验证 Apple 代码签名/公证；哈希一致只证明与发布方字节一致，不证明软件安全。
- agent/src/core_download.rs 当前自动下载流程按最新 release 查找并直接安装，不替代这里的固定版本核验。个人构建应使用上述固定核验文件，不依赖“最新”按钮。

## 本环境阻塞
内核构建脚本明确要求 macOS。fetch.sh 默认至少 150 GB 空闲，说明约 100 GB 同步、200 GB 完成构建。此云机容量不足且不是 macOS；不能声称源码内核构建或 Mac App 构建完成。

官方仓库有 self-hosted.yml 浏览器测试，但依赖预先具备 Chromium 树的 fury-core runner，不是给本任务提供的构建机。CI 只做只读审查，未触发。遵守不使用 GitHub Actions 的限制。

## 后续已准备的命令
在未来明确授权的 Apple Silicon macOS 构建机，安装 Xcode/SDK、原生 arm64 Rust、Node/npm、Python，具备依赖联网条件后：

```bash
bash build-personal-macos-arm64.sh /path/to/fury-source
# 若具备至少 200 GiB 空间且要源码内核：
bash build-personal-macos-arm64.sh /path/to/fury-source --full-core
```

脚本使用现有 Tauri、sidecar 和 Chromium 构建路径；不部署、不安装、不上传、不修改钥匙串；不调用上游全局清理 /Volumes/dmg.* 的逻辑。输出 app zip 而非 DMG，保留签名检查输出但不做 Developer ID 签名/公证。正式可分发包需另行授权的证书与公证流程。没有证书不妨碍本地源码构建，但不能把本地产物宣称为已签名发行版。

## 许可与持久化
上游分层许可保持原样：AGPL-3.0-or-later 应用/agent/server 等；Chromium 派生补丁 BSD-3-Clause；schemas Apache-2.0；Chromium 自身遵循其上游许可。未创建或推送远程仓库。云文件不能视作永久备份，应与修改源码、测试结果一并送入可取回交付附件。

## 固定内核下载与安装分界

```bash
python3 tools/personal/fetch-pinned-core.py --output-dir /path/to/downloads
# 已有文件可离线验证：
python3 tools/personal/fetch-pinned-core.py --verify-only /path/to/downloads/fury-core-0.5.7-macos-arm64.tar.xz
```

该辅助程序只下载并检查固定长度及 SHA256，不安装、不解包、不执行。已在云端使用真实下载文件通过验证，同时验证拒绝错误长度及相同长度但错误哈希文件。Mac 环境中可再由构建后的 agent 安装已核验文件：

```bash
target/release/fury-agent install-core /path/to/downloads/fury-core-0.5.7-macos-arm64.tar.xz
```

安装命令是将来的明确本机安装步骤，本次未执行。上游 installer 会试运行浏览器；不能把静态哈希检查当作其启动成功。应用和内核独立保存，脚本未将内核写进 app bundle。下载来源与哈希固定不意味着自动获得未来安全更新，更新应重新审查新版并改 pin。
