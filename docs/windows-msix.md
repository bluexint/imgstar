# Windows MSIX 打包

仓库已补齐基于 winapp CLI 的 MSIX 打包脚本，流程对齐 Microsoft 的 Tauri 指南。

## 先决条件

- Windows 11
- Node.js
- Rust 工具链
- winapp CLI

## 一键打包

```powershell
npm.cmd run package:msix
```

脚本会自动完成这些步骤：

1. 调用 Tauri 生成 release 版本可执行文件。
2. 在 `dist/msix` 下生成 Appx manifest 和临时签名证书。
3. 使用 winapp CLI 打包并输出 `dist/msix/imgstar.msix`。

## 本机安装证书

如果需要在本机双击安装后运行，可以用管理员 PowerShell 安装证书：

```powershell
winapp cert install .\dist\devcert.pfx
```

## 备注

- 生成产物都放在 `dist/` 下，仓库默认忽略这些内容。
- 当前 Tauri 应用标识已更新为 `com.imgstar.desktop`，避免继续使用占位符。