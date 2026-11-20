# HomeworkInfoSync 安卓版

独立 Android 应用（包名 `io.github.e7g.homeworksync`），保留 Rust 同步核心，通过 JNI 接入 Android 原生界面。不需要在手机上运行 Qt，也不需要部署服务器。

## 安装和使用

1. 在 Android 8.0（API 26）及以上的设备上安装与 CPU 架构匹配的签名 APK。
2. 打开「作业同步」→「平台」。超星 / 学习通使用账号密码，课堂派使用邮箱和密码。
3. 长江雨课堂支持微信扫码：可用另一台设备扫描，也可截图后用微信「扫一扫」从相册选取。扫码不便时可以手动填写已有登录凭证。
4. 「作业」页按截止时间展示结果，可切换作业状态、筛选平台并跳转原平台。应用启动时读取本机缓存并发起同步，也可点「同步」手动刷新；不在应用关闭后定时运行。
5. 登录信息通过 Android Keystore 加密，作业缓存仅保存在应用私有目录，不参与备份。断开平台时会清除对应的登录信息和缓存。
6. 点顶栏「外观」可选择跟随系统（默认）、浅色或深色。选择会保存在本机，切换主题不会中断同步。

此版本沿用上游项目使用的平台接口及覆盖范围。学校或平台的登录要求可能不同；请在「同步记录」查看实际同步结果。

## 构建（Windows）

环境：PowerShell 7、JDK 17+、Rust stable 1.93+、`cargo-ndk`、Android SDK Platform 35、Build Tools 35.0.0、NDK 28.2.13676358（或通过 `ANDROID_NDK_HOME` 指定 r28 或更新的版本）。
设置 `JAVA_HOME` 和 `ANDROID_HOME`，确保 `cargo` 在 PATH 中。

```powershell
rustup target add aarch64-linux-android
cargo install cargo-ndk --locked
.\homework-android\build-apk.ps1
```

默认只打包 `arm64-v8a`。如需其他架构，先安装 Rust `armv7-linux-androideabi` 和/或 `x86_64-linux-android` 目标，然后传入 `-Abi arm64-v8a,armeabi-v7a,x86_64`。构建结果位于 `homework-android/build/HomeworkInfoSync.apk`。

脚本默认生成并重用本机开发签名 `homework-android/build/local-signing.p12`。分发自建版本时，请在首次打包前设置自用签名：设置环境变量 `HOMEWORK_KS_PASS` 并传入 `-KeyStore` 和 `-KeyAlias`。同包名覆盖更新需要沿用首次安装的签名，源码包不含签名私钥。

## 代码结构

- `app/`：原生 Android Activity、平台配置、加密存储和本地缓存。
- `native/`：从 Android 应用调用原有 Rust 平台客户端、二维码和作业统计的 JNI 层。
- `../homework-core/`：与桌面版共用的作业同步核心。

基于 [E7G/HomeworkInfoSync](https://github.com/E7G/HomeworkInfoSync) 的 MIT 许可代码，保留原项目许可证。
