# HomeworkInfoSync

多平台作业信息同步工具，自动获取超星/学习通、课堂派、长江雨课堂的作业信息，统一展示截止时间和提交状态。

## 功能

- **多平台支持**：超星/学习通、课堂派、长江雨课堂
- **GUI 界面**：基于 PyQt6 的深色主题界面，支持作业卡片展示、紧急程度标记
- **扫码登录**：长江雨课堂支持微信扫码登录，凭证自动保存
- **命令行模式**：支持终端输出作业提醒，方便脚本调用

## 截图

> 运行 `pixi run gui` 启动 GUI 界面

## 快速开始

### 环境要求

- [pixi](https://pixi.sh/) 包管理器

### 安装

```bash
git clone https://github.com/E7G/HomeworkInfoSync.git
cd HomeworkInfoSync
pixi install
```

### 配置

复制示例配置文件并填写账号信息：

```bash
cp config.example.json config.json
```

编辑 `config.json`，填入各平台账号：

```json
{
  "chaoxing": {
    "enabled": true,
    "user": "超星账号",
    "password": "超星密码"
  },
  "ketangpai": {
    "enabled": true,
    "email": "课堂派邮箱",
    "password": "课堂派密码"
  },
  "yuketang": {
    "enabled": false,
    "csrftoken": "",
    "sessionid": "",
    "university_id": "3078"
  }
}
```

长江雨课堂推荐通过 GUI 界面扫码登录，无需手动填写凭证。

### 运行

```bash
# GUI 模式
pixi run gui

# 命令行模式
pixi run remind
```

## 打包

```bash
pixi run pyinstaller --noconfirm --onefile --windowed --name HomeworkSync --add-data "config.example.json;." gui.py
```

生成的可执行文件在 `dist/` 目录下。

## 发布

推送 `v*` 格式的 tag 即可触发 GitHub Actions 自动构建并发布到 Release：

```bash
git tag v0.1.0
git push origin v0.1.0
```

## 致谢

本项目开发过程中参考了以下开源项目，感谢它们的贡献：

- [Raincourse](https://github.com/aglorice/Raincourse) — 雨课堂作业相关实现参考（MIT License）
- [yuketangHelperBUU](https://github.com/SSRSH/yuketangHelperBUU) — 长江雨课堂 WebSocket 扫码登录实现参考
- [chaoxing-list](https://github.com/Cooanyh/chaoxing-list) — 超星/学习通作业与考试列表实现参考（AGPL-3.0）
- [ketangpai-content-gripper](https://github.com/JiangGe-Ch/ketangpai-content-gripper) — 课堂派 API 调用实现参考（Apache-2.0）
- homeworkHelper.py — 雨课堂作业获取实现参考（zk chen & MR.Li）

## 许可证

[MIT](LICENSE)
