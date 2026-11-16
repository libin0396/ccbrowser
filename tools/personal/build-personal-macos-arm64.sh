#!/usr/bin/env bash
# Personal build wrapper; no install, upload, keychain changes, or CI invocation.
set -euo pipefail
repo="${1:?usage: build-personal-macos-arm64.sh /path/to/fury-source [--full-core]}"
repo="$(cd "$repo" && pwd)"
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || { echo 'Requires native Apple Silicon macOS build host.' >&2; exit 2; }
for tool in xcodebuild xcrun rustc cargo node npm python3; do command -v "$tool" >/dev/null || { echo "Missing $tool" >&2; exit 2; }; done
xcodebuild -version
xcrun --sdk macosx --show-sdk-path
[[ "$(rustc -vV | sed -n 's/^host: //p')" == aarch64-apple-darwin ]] || { echo 'Native arm64 Rust required; do not build under Rosetta.' >&2; exit 2; }
[[ -z "${CARGO_TARGET_DIR:-}" ]] || { echo 'Unset CARGO_TARGET_DIR: upstream sidecar expects repo/target.' >&2; exit 2; }
cd "$repo"
test -f Cargo.lock && test -f desktop/package-lock.json
mkdir -p personal-artifacts
if [[ "${2:-}" == --full-core ]]; then
  [[ $(df -Pk . | awk 'NR==2 {print int($4/1048576)}') -ge 200 ]] || { echo 'Full core build requires at least 200 GiB free; upstream estimates ~200 GB total.' >&2; exit 2; }
  version="$(cat core/CHROMIUM_VERSION)"
  core/build/fetch.sh "$version"
  core/build/apply.sh
  core/build/link-icons.sh
  core/build/build.sh macos-arm64
elif [[ -n "${2:-}" ]]; then
  echo 'Unknown mode' >&2; exit 2
fi
# No automatic use of signing credentials. Local unsigned/ad-hoc build only.
unset APPLE_SIGNING_IDENTITY APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_KEY APPLE_API_ISSUER
(cd desktop && npm ci && npm run build)
cargo test --locked -p fury-agent
# Keep the locked Rust dependency graph. Upstream sidecar hook is rerun by Tauri.
cargo build --locked --release -p fury-agent
(cd desktop && npm run tauri -- build --bundles app)
test -d target/release/bundle/macos/Fury.app
file target/release/bundle/macos/Fury.app/Contents/MacOS/*
ditto -c -k --sequesterRsrc --keepParent target/release/bundle/macos/Fury.app personal-artifacts/Fury-personal-macos-arm64.zip
shasum -a 256 personal-artifacts/Fury-personal-macos-arm64.zip > personal-artifacts/SHA256SUMS
printf 'Personal local build; no Developer ID signing or notarization was performed by this wrapper. Browser core is separate.\n' > personal-artifacts/STATUS.txt
codesign -dv --verbose=4 target/release/bundle/macos/Fury.app 2> personal-artifacts/codesign.txt || true
printf 'Artifacts: %s/personal-artifacts\n' "$repo"
