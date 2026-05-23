# HomeworkInfoSync

多平台作业信息同步工具，自动获取超星/学习通、课堂派、长江雨课堂的作业信息，统一展示截止时间和提交状态。

## 功能

- **多平台支持**：超星/学习通、课堂派、长江雨课堂
- **Qt Widgets 图形界面（Rust + Qt6）**：原生 QWidget，冷启动快；打开即显示本地缓存，后台静默刷新
- **扫码登录**：长江雨课堂支持微信扫码登录，凭证自动保存
- **命令行模式**：`homework-remind` 适合脚本调用

## 环境要求

- [Rust](https://rustup.rs/) 1.75+
- **Qt 6**（含 Widgets、Gui 模块）
  - Windows：安装 [Qt Online Installer](https://www.qt.io/download-qt-installer) 或 `winget install Qt.Qt.6`
  - 设置环境变量 `CMAKE_PREFIX_PATH` 指向 Qt 安装目录（例如 `C:\Qt\6.8.0\msvc2019_64`）

## 快速开始

```bash
git clone https://github.com/E7G/HomeworkInfoSync.git
cd HomeworkInfoSync

# 编译 GUI（Release 推荐，体积更小、启动更快）
cargo build --release -p homework-app

# 复制示例配置到 exe 同目录
cp config.example.json target/release/config.json

# 运行
./target/release/HomeworkSync.exe   # Windows
```

首次打开会立即显示 `homework_cache.json` 中的缓存；已配置的平台会在后台自动刷新。

### 配置

复制 `config.example.json` 为 `config.json`，填写各平台账号。程序会按顺序查找：

1. 环境变量 `HOMEWORK_CONFIG` 指定的路径
2. 从可执行文件所在目录向上逐级查找 `config.json`（`cargo run` 时可找到仓库根目录的配置）
3. 从当前工作目录向上查找

发布版建议将 `config.json` 放在 `HomeworkSync.exe` 同目录。长江雨课堂推荐在 GUI 配置页扫码登录。

### 命令行

```bash
cargo run --release -p homework-core --bin homework-remind
```

## 发布

推送 `v*` 标签触发 GitHub Actions 构建 Windows Release 包：

```bash
git tag v0.2.0
git push origin v0.2.0
```

## 致谢

- [new_xxt](https://github.com/aglorice/new_xxt) — 超星/学习通作业完整链接获取思路参考
- [Raincourse](https://github.com/aglorice/Raincourse)
- [yuketangHelperBUU](https://github.com/MuWinds/yuketangHelperBUU)
- [chaoxing-list](https://github.com/Cooanyh/chaoxing-list)
- [ketangpai-content-gripper](https://github.com/JiangGe-Ch/ketangpai-content-gripper)
- [yuketangHelper](https://github.com/heyblackC/yuketangHelper)

## 许可证

[MIT](LICENSE)
