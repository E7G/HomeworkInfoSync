# HomeworkInfoSync

原生安卓移植见 [homework-android](homework-android/README.md)。安装包位于 [GitHub Releases](https://github.com/E7G/HomeworkInfoSync/releases/latest)。

多平台作业信息同步工具，自动获取超星/学习通、课堂派、长江雨课堂的作业信息，统一展示截止时间和提交状态。

## 功能

- **多平台支持**：超星/学习通、课堂派、长江雨课堂
- **Qt Widgets 图形界面（Rust + Qt6）**：原生 QWidget，冷启动快；打开即显示本地缓存，后台静默刷新
- **扫码登录**：长江雨课堂支持微信扫码登录，凭证自动保存
- **命令行模式**：`homework-remind` 适合脚本调用
- **统计说明**：每次刷新后，在「日志」页底部附带与首页一致的数字说明（含分项核对、按平台分解），便于理解与排错

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
# 在提醒列表后打印与 GUI 日志相同的「统计说明」全文
cargo run --release -p homework-core --bin homework-remind -- --stats
```

## 发布

在 [Actions → Build and Release](https://github.com/E7G/HomeworkInfoSync/actions/workflows/release.yml) 点击 **Run workflow**，选默认分支并输入未发布的版本标签（如 `v0.2.0`）。流程会编译 Windows ZIP 和签名 Android APK，两者都通过后上传到同一个 GitHub Release，无需手动上传。

也可以在默认分支的提交上推送版本标签触发：

```bash
git tag v0.2.0
git push origin v0.2.0
```

其他仓库要启用发布，请在 Actions 设置中配置 `HOMEWORK_ANDROID_KEYSTORE_BASE64` 和 `HOMEWORK_ANDROID_KEYSTORE_PASSWORD` 两个 Secrets，以及 `HOMEWORK_ANDROID_CERT_SHA256` Variable。签名文件须为带 `homework-sync` 别名的 PKCS#12 文件，签名私钥不进仓库。后续安卓发布打包前请更新 AndroidManifest 中的版本号；为支持覆盖安装，请沿用同一签名。

## 致谢

- [new_xxt](https://github.com/aglorice/new_xxt) — 超星/学习通作业完整链接获取思路参考
- [Raincourse](https://github.com/aglorice/Raincourse)
- [yuketangHelperBUU](https://github.com/MuWinds/yuketangHelperBUU)
- [chaoxing-list](https://github.com/Cooanyh/chaoxing-list)
- [ketangpai-content-gripper](https://github.com/JiangGe-Ch/ketangpai-content-gripper)
- [yuketangHelper](https://github.com/heyblackC/yuketangHelper)

## 许可证

[MIT](LICENSE)
