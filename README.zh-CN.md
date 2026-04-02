# Imgstar

Imgstar 是一个基于 Tauri v2 的 Windows 桌面应用，用来管理图片等静态资源的上传、删除、CDN 刷新和 WAF allowlist 同步。仓库采用 Rust + Vue + TypeScript 的 monorepo 结构，后端负责对象存储与 Cloudflare 交互，前端负责配置、预览和操作入口。

## 关于项目

- Tauri v2 桌面架构，前端是 Vue 3 + TypeScript，后端是 Rust，适合做本地化、低体积的桌面工具。
- 支持 S3 兼容对象存储上传与删除，能对接 Cloudflare R2 或其他兼容 S3 的存储服务。
- 集成 Cloudflare CDN 刷新与 WAF allowlist 同步，适合图片资源类站点的发布流程。
- 具备绕过路径与特殊字符过滤，能降低 `//`、`..`、`%2e%2e`、`^$;%?#=` 等变形输入带来的风险，来降低cdn缓存绕过攻击造成不必要的天价账单
- 项目按职责拆分为 `apps/desktop`、`packages/contracts`、`src-tauri` 和 `scripts`，便于维护和扩展。
- 面向 Windows 的打包链路已经整理好，可直接产出 MSIX 安装包。

## 使用教程

### 1. 安装依赖

在仓库根目录执行：

```powershell
npm.cmd install
```

如果你想一次性检查本机环境是否满足要求，可以执行：

```powershell
npm.cmd run env:check
```

### 2. 配置本地环境

需要的基础环境：

- Node.js
- Rust 工具链
- Tauri CLI
- Windows 下的 `winapp` CLI（如果要打 MSIX）

如果你还没有把本机环境准备好，可以运行：

```powershell
npm.cmd run env:setup
```

### 3. 启动开发模式

```powershell
npm.cmd run tauri dev
```

开发模式下，前端和 Rust 后端会一起启动，适合调试上传、WAF 同步和 Cloudflare 相关功能。

### 4. 常用检查命令

```powershell
npm.cmd run check
```

这会依次执行 lint、typecheck 和前端测试，适合在提交前快速确认代码状态。

## 代码编译教程

### 1. 前端与类型检查

```powershell
npm.cmd run lint
npm.cmd run typecheck
npm.cmd run test
```

如果你只想跑桌面端的单元测试：

```powershell
npm.cmd --workspace @imgstar/desktop run test:unit
```

### 2. 后端验证

Rust 后端的核心逻辑在 `src-tauri` 下，常用验证命令是：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml runtime::adapter_runtime
```

### 3. 构建桌面应用

```powershell
npm.cmd run build
```

这个命令会走 `scripts/build-tauri.ps1`，生成可分发的桌面构建产物。

### 4. 打包 MSIX

如果你需要 Windows 安装包：

```powershell
npm.cmd run package:msix
```

打包流程会输出到 `dist/msix`，并生成最终的 `.msix` 文件。

## 目录说明

- `apps/desktop`：桌面端前端代码、页面、测试与样式。
- `packages/contracts`：前后端共享的类型与契约定义。
- `src-tauri`：Rust 后端、对象存储、Cloudflare、WAF 和运行时逻辑。
- `scripts`：Windows 下的环境准备、构建和打包脚本。
- `docs`：架构说明和 Windows/MSIX 相关文档。
