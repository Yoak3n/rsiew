# Rsiew

一个简化的 [Work Review](https://github.com/wm94i/Work_Review) 克隆应用，具有原生图标和系统托盘功能。这是一个使用 Tauri（Rust 后端）和 Vue.js（前端）构建的桌面应用程序，用于跟踪工作活动和生产力。

主要是为了[aimin](https://github.com/Yoak3n/aimin)中的`Watch`功能需要cli这个饺子而添的醋，~~可以说是纯私人定制~~，更丰富优秀的类似功能推荐[Work Review](https://github.com/wm94i/Work_Review)

## 功能特性

- **活动监控**：跟踪应用程序使用情况和工作动态
- **截图功能**：内置截图能力
- **OCR 支持**：图像处理的 OCR 功能
- **数据库存储**：使用 SQLite 存储使用统计
- **系统托盘集成**：带菜单的系统托盘图标
- **跨平台支持**：支持 Windows、macOS 和 Linux
- **CLI 界面**：用于查询统计信息的命令行界面
- **GUI 界面**：基于 Vue.js 的现代用户界面

## 技术栈

基于[Tauri2](https://github.com/tauri-apps/tauri)跨平台桌面应用程序框架，使用Rust+Vue.js构建，支持Windows、macOS和Linux（小字：支持macOS和Linux是设计目标）。

## 开发

### 开发环境

- **Rust**：从 [rust-lang.org](https://www.rust-lang.org/) 安装
- **Node.js**：从 [nodejs.org](https://nodejs.org/) 安装
- **pnpm**：包管理器 (`npm install -g pnpm`)

### 开发环境设置

1. 克隆仓库：
   ```bash
   git clone <仓库地址>
   cd rsiew
   ```

2. 安装前端依赖：
   ```bash
   pnpm install
   ```

3. 安装 Rust 依赖（由 Cargo 自动处理）

4. 以开发模式运行：
   ```bash
   pnpm tauri dev
   ```

## 构建

### 开发构建
```bash
pnpm tauri build
```

### 生产构建
```bash
pnpm tauri build --release
```

## 使用方法

### GUI 应用程序
运行应用程序：
```bash
pnpm tauri dev
```

### CLI 界面
应用程序包含命令行界面：

```bash
# 查询今日统计
rsiew-cli stats

# 查询周统计
rsiew-cli stats week

# 查询自定义时间范围
rsiew-cli stats --start 1700000000 --end 1700086400
```

## 配置

应用程序配置通过以下方式管理：
- `src-tauri/tauri.conf.json` - Tauri 应用程序配置
- `src-tauri/tauri.windows.conf.json` - Windows 特定配置
- 数据库存储在用户的数据目录中

## 开发

### 版本管理
项目包含版本管理脚本：

```bash
# 补丁版本 (0.1.5 → 0.1.6)
pnpm version:patch

# 次要版本 (0.1.5 → 0.2.0)
pnpm version:minor

# 主要版本 (0.1.5 → 1.0.0)
pnpm version:major
```

### 代码结构
- **前端**：使用 Composition API 和 TypeScript 的 Vue 3
- **后端**：使用 Tauri 2 框架的 Rust
- **数据库**：使用 Rusqlite ORM 的 SQLite
- **构建**：前端使用 Vite，Rust 使用 Cargo

## 许可证

本项目根据仓库中包含的条款进行许可。

## 贡献指南

1. Fork 本仓库
2. 创建功能分支
3. 进行更改
4. 提交 Pull Request

## 致谢

- 使用 [Tauri](https://tauri.app/) 构建
- 前端由 [Vue.js](https://vuejs.org/) 提供支持
- 图标和资源来自 Tauri 模板