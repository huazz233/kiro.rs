---
name: version-bump
description: 升级项目版本号并提交git，支持patch/minor/major版本升级或指定具体版本号
version: 1.0.0
author: https://github.com/BenedictKing/kiro.rs/
allowed-tools: Bash, Read, Write, Edit
context: fork
---

# 版本号升级技能

## 指令

当用户输入包含以下关键词时，自动触发版本升级流程：

### 中文触发条件

- "升级版本"、"版本号"、"发布版本"、"更新版本"、"更新版本并提交"、"提交 git" → 执行版本升级
- "bump"、"release" → 执行版本升级

### 参数说明

- 无参数或 `patch`: patch 版本 +1
- `minor`: minor 版本 +1, patch 归零
- `major`: major 版本 +1, minor 和 patch 归零
- 具体版本号 (如 `2.1.0`): 直接使用该版本号

### 发布选项

> ⚠️ **重要**: 默认情况下，版本升级后必须创建 tag 并推送！只有推送 tag 才能触发 GitHub Actions 自动编译发布。除非用户明确说"不要 tag"或"--no-tag"，否则始终创建并推送 tag。

- `--no-tag` 或 "不要 tag": 不创建 git tag（仅提交版本变更）
- `--push` 或 "并推送"、"push": 推送 commit 到远程仓库（默认行为）

## 版本号修改位置（必须全部同步更新）

> ⚠️ **每次版本升级必须同时修改以下所有位置，缺一不可！**

| # | 文件 | 字段/内容 | 格式 | 示例 |
|---|------|-----------|------|------|
| 1 | `VERSION` | 整个文件内容 | `v{major}.{minor}.{patch}` | `v1.0.1` |
| 2 | `Cargo.toml` | `version = "..."` (第3行) | `{major}.{minor}.{patch}` (无 `v` 前缀) | `"1.0.1"` |
| 3 | `CHANGELOG.md` | `## [Unreleased]` → `## [v{版本}] - 日期` | `[v{major}.{minor}.{patch}] - YYYY-MM-DD` | `[v1.0.1] - 2026-02-08` |

## 执行步骤

### 1. 读取当前版本号

```bash
cat VERSION
```

### 2. 解析并计算新版本号

根据用户指定的升级类型计算：

| 当前版本 | 升级类型     | 新版本  |
| -------- | ------------ | ------- |
| v1.0.0   | patch (默认) | v1.0.1  |
| v1.0.0   | minor        | v1.1.0  |
| v1.0.0   | major        | v2.0.0  |
| v1.0.0   | 2.1.5        | v2.1.5  |

### 3. 更新版本文件

```bash
# 更新 VERSION 文件
echo "v{新版本号}" > VERSION

# 更新 Cargo.toml 中的 version 字段（不带 v 前缀）
# 使用 Edit 工具将 version = "旧版本" 替换为 version = "新版本"
```

### 4. 更新 CHANGELOG.md

将 `[Unreleased]` 替换为新版本号和当前日期：

```markdown
# 替换前

## [Unreleased]

# 替换后

## [v{新版本号}] - YYYY-MM-DD
```

### 5. 验证更新

```bash
cat VERSION
grep '^version' Cargo.toml
cat CHANGELOG.md | head -20
```

### 6. 查看 git 状态

```bash
git status
git diff --stat
```

### 7. 提交变更

询问用户确认提交信息后执行：

```bash
git add -A
git commit -m "chore: bump version to v{新版本号}"
```

### 8. 创建 Tag（默认必须执行）

> ⚠️ 除非用户明确说"不要 tag"，否则必须创建 tag！

```bash
git tag v{新版本号}
```

### 9. 推送到远程（默认必须执行）

```bash
# 推送 commit
git push origin master

# 推送 tag（触发 GitHub Actions 自动编译发布）
git push origin v{新版本号}
```

## 示例场景

### 场景 1：默认升级 patch 版本

**用户输入**: "升级版本号并提交" 或 "更新版本并推送"

**自动执行流程**:

1. 读取 VERSION: `v1.0.0`
2. 计算新版本: `v1.0.1`
3. 更新 VERSION 文件和 Cargo.toml
4. 执行 git commit
5. 创建 git tag: `v1.0.1`
6. 推送 commit 和 tag 到远程

### 场景 2：发布新版本（完整流程）

**用户输入**: "发布新版本并打 tag 推送"

**自动执行流程**:

1. 读取 VERSION: `v1.0.0`
2. 计算新版本: `v1.0.1`
3. 更新 VERSION 文件和 Cargo.toml
4. 更新 CHANGELOG.md
5. 执行 git commit
6. 创建 git tag: `v1.0.1`
7. 推送 commit 和 tag 到远程
8. GitHub Actions 自动触发，编译 6 平台版本并发布到 Releases

## 输出格式

### 基础版本升级

```
版本升级完成:
- 原版本: v1.0.0
- 新版本: v1.0.1
- 升级类型: patch

是否提交 git? (Y/n)
```

### 完整发布流程

```
版本升级完成:
- 原版本: v1.0.0
- 新版本: v1.0.1
- 升级类型: patch

✅ Git commit 已创建
✅ Git tag v1.0.1 已创建

是否推送到远程仓库? (Y/n)
  - 推送后将自动触发 GitHub Actions
  - 自动编译 Linux/Windows/macOS 版本
  - 自动发布到 GitHub Releases
```

## GitHub Actions 集成

当推送 `v*` 格式的 tag 时，会自动触发以下 workflow：

| Workflow              | Runner         | 产物                                                                 |
| --------------------- | -------------- | -------------------------------------------------------------------- |
| `release-linux.yml`   | ubuntu-latest  | `kiro-rs-linux-amd64`, `kiro-rs-linux-arm64`                         |
| `release-macos.yml`   | macos-latest   | `kiro-rs-darwin-arm64`, `kiro-rs-darwin-amd64`                       |
| `release-windows.yml` | windows-latest | `kiro-rs-windows-amd64.exe`, `kiro-rs-windows-arm64.exe`            |
| `docker-build.yml`    | ubuntu-latest  | Docker 镜像 (阿里云容器镜像服务, linux/amd64 + linux/arm64)          |

## 注意事项

- 版本号格式为 `v{x}.{y}.{z}`（无后缀）
- **VERSION 和 Cargo.toml 必须同步更新**
- 提交前会显示所有待提交的变更供用户确认
- 遵循 Conventional Commits 规范，使用 `chore: bump version` 格式
- 默认分支为 `master`
- 推送 tag 后，GitHub Actions 需要几分钟完成编译和发布
