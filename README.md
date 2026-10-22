<div align="center">

# 应用卸载器

**卸得干净，删得放心。**

</div>

<div align="center">

[![Release](https://img.shields.io/github/v/release/ducaoya/app-uninstaller?color=0a84ff&label=version)](https://github.com/ducaoya/app-uninstaller/releases/latest)
[![Build](https://github.com/ducaoya/app-uninstaller/actions/workflows/release.yml/badge.svg)](https://github.com/ducaoya/app-uninstaller/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/license-MIT-30d158.svg)](./LICENSE)
![Platform](https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-0078d4)

</div>

---

## 它解决什么

卸载软件从来都是两步：系统卸载程序跑完，然后你自己满硬盘找残余——注册表里一堆空键、AppData 里几个 G 的缓存、开始菜单里失效的快捷方式。

应用卸载器把第二步自动化：系统卸载一完成，立刻扫描所有残余并清理干净；碰上卸载器损坏的流氓软件，还能直接强制卸载。

## 主要能力

| | 能力 | 说明 |
|---|---|---|
| 📋 | **一屏看全** | 自动列出系统全部已安装应用，含系统级、32 位、当前用户三类来源，版本 / 发布者 / 占用空间一目了然 |
| 🖼 | **真实图标** | 从程序本身提取图标，认不出图的才用字母占位 |
| 🗑 | **标准卸载** | 唤起系统自带卸载程序，结束后自动扫描注册表、数据目录、快捷方式三类残余并清理 |
| 💥 | **强制卸载** | 卸载器罢工时直接结束进程、删除文件与注册表、清理残留；安装包类软件自动走系统静默卸载，不留孤儿记录 |
| 🧹 | **单项清理** | 不想整个卸载？也可以只清注册表、只清缓存文件夹 |
| ⏪ | **删错可反悔** | 数据目录与快捷方式删除进回收站，随时找回；注册表删除逐条记录在日志 |
| ⏱ | **实时日志** | 每一步操作实时滚动显示，带耗时统计，长任务可随时取消 |
| 🔒 | **权限克制** | 只清理与目标应用匹配的残余，系统保护路径绝不误删 |

## 使用方式

1. 双击运行，Windows 会请求管理员权限（清理系统级残留需要），点「是」
2. 在列表中找到应用，**右键**选择操作：
   - **卸载** —— 走系统卸载流程，完成后自动清理残留
   - **强制卸载** —— 直接删除（不可恢复，会先弹确认）
   - **清除注册表** —— 删除该应用的注册表键
   - **清除缓存文件夹** —— 清理 AppData 下的数据目录
3. 底部状态条实时显示当前进度，点开可查看完整日志与每步耗时

> 首次运行弹出的「未知发布者」提示是因为程序未做商业签名，属正常现象。

## 下载

前往 [Releases](https://github.com/ducaoya/app-uninstaller/releases/latest) 下载最新的 `App.Uninstaller_x.y.z_x64-setup.exe`，双击安装即可。

需要 Windows 10 / 11（64 位）。

## 安全

- 标准 / 强制卸载前都会先结束应用进程，避免文件占用
- 数据目录与快捷方式删除进**回收站**，注册表删除**全部记录在日志**
- 命中系统保护路径（Windows、Program Files 等）时自动跳过

## 许可

[MIT](./LICENSE) © 2026 ducaoya &lt;ducaoya@gmail.com&gt;
