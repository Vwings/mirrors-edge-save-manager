# Mirror's Edge 存档管理器

[![许可证：GPL-3.0-only](https://img.shields.io/badge/license-GPL--3.0--only-blue.svg)](LICENSE)
[![平台：Windows](https://img.shields.io/badge/platform-Windows%2010%2F11-0078d4.svg)](#运行要求)

[English](README.md) · [简体中文](README.zh-CN.md)

![Mirror's Edge 存档管理器](resources/header-logo.png)

一个非官方的 Windows 工具，帮你整理并安全切换《Mirror's Edge》（2008）PC 版存档。

## 它能做什么

玩《Mirror's Edge》时，你可能会想留住一个通关进度，试试别人的存档，或者随时回到之前的状态。这个工具就是为这些场景准备的：把存档集中收好，需要时安全切换，不必再手动寻找和复制 `.dat` 文件。

在程序里，一个存档文件会扮演三种角色。它们本质上都是同一种游戏 `.dat` 存档，区别在于是否正在被游戏使用，以及你准备如何使用它：

- **Current**：游戏当前正在使用的存档，保留在游戏的存档目录中。
- **Preset（预设）**：保存下来、准备反复使用的存档起点。
- **Stash（暂存）**：保存下来、用于备份或记录历史状态的存档。每次切换前会自动生成一份。

选择一个 Preset 或 Stash 后，程序会先把 Current 保存成 Stash，再完成切换并检查结果。被选择的副本不会被覆盖，因此你可以放心尝试不同存档，也能随时从 Stash 回到之前的状态。

程序不会改动存档内容，只负责复制、校验、压缩和恢复完整文件。

### 内置预设

程序提供四个只读预设，分别对应常见的游戏起点：

- **New Game（新游戏）**：全新的开始，适合重新体验完整流程。
- **Completed Campaign（完成主线）**：主线流程已经完成，可以直接进入通关后的状态。
- **69% Speedrun（69% 速通）**：速通社区用于 69% 项目的起始存档。
- **All Time Trials Unlocked（解锁全部时间试炼）**：已完成主线并解锁时间试炼，适合练习计时挑战。

内置预设随程序提供，不能编辑或删除，目前会直接显示在预设列表中。

## 功能

- 自动找到游戏存档目录和当前存档。
- 将当前存档保存为预设或暂存，并添加名称和描述。
- 导入外部 `.dat` 文件作为预设。
- 在预设和暂存之间安全切换，原副本保持不变。
- 切换前自动备份当前存档。
- 操作前后检查文件完整性，确保存档没有损坏。
- 操作中断时自动恢复，避免丢失原存档。
- 发现内容重复时提醒你确认。
- 提供四个常用起点的只读内置预设。
- 支持英文和简体中文，选择会自动保存。

## 运行要求

- Windows 10 或 Windows 11，x64
- PC 版《Mirror's Edge》
- 对游戏存档位置具有写入权限

第一次使用前，请先启动一次游戏，让游戏创建存档目录。程序会自动找到
Windows 的 **Documents** 文件夹，并使用游戏标准的
`EA Games\\Mirror's Edge\\TdGame\\Savefiles\\` 目录。

## 安装与启动

从仓库的 [Releases](https://github.com/Vwings/mirrors-edge-save-manager/releases) 页面下载 Windows 可执行文件，放到方便的位置后直接运行即可，无需安装程序。

用户数据会独立保存于：

```text
%LOCALAPPDATA%\\Mirror's Edge Save Manager\\
```

每个存档副本都会单独放在自己的目录中：

```text
%LOCALAPPDATA%\\Mirror's Edge Save Manager\\
├─ stored-saves\\<id>\\metadata.json
├─ stored-saves\\<id>\\payload.dat.gz
├─ transactions\\<id>.json
└─ settings.json
```

`metadata.json` 保存名称、描述和校验信息，`payload.dat.gz` 是压缩后的存档副本，
`transactions` 用于中断操作后的恢复，`settings.json` 保存语言等程序设置。

## 日常使用流程

1. 修改存档前先关闭游戏。游戏运行时，相关操作会自动暂停。
2. 打开管理器，先查看 **Current**，确认当前存档状态。
3. 点击 **Save as Stash** 留一份备份，或点击 **Save as Preset** 保存一个常用起点。
4. 点击 **Import .dat** 导入外部存档，填写名称和可选描述；原文件不会被改动。
5. 选择预设或暂存并点击 **Apply**。程序会先自动备份 Current，再应用所选存档。
6. 在 **Stash** 标签中找回历史状态，也可以用 **Make Preset** 把暂存变成预设。

如果存档目录已经存在但找不到 Current，程序会先请你确认根据账户名生成的
`<username>.dat` 文件名，再创建新的 Current。目录里的其他 `.dat` 文件会保留原样。

## 安全机制

每次切换存档前，程序都会确认游戏已经关闭，并锁定当前操作，避免多个管理器同时修改。新文件会先在旁边准备并校验，同时保留一份回滚副本；确认替换后的文件可以正常读取后，操作才算完成。出现问题时，原来的 Current 会保留或自动恢复。

程序不会在没有备份的情况下删除 Current。建议仍保留自己的重要备份。

## 故障排查

**Apply 被禁用** —— 关闭游戏并刷新窗口。如果原生存档目录不存在，请先启动一次游戏，再回到管理器。

**替换失败** —— 关闭游戏及可能占用存档的工具。程序提示恢复期间，不要删除事务文件。

**提示需要恢复** —— 先不要移动或删除界面列出的文件，按提示完成恢复。为保护存档，无法确认安全状态时程序会暂停修改操作。

**想切换语言** —— 点击顶部的 `EN` 或 `中文` 即可，选择会自动保存。

## 项目范围

- 目前支持 Windows x64。
- 存档以完整文件复制和恢复，进度编辑不在项目范围内。
- 暂存会一直保留，直到手动删除。
- 内置预设为只读源文件。

## 参与开发

开发命令、安全边界、本地化流程和发布验证请参阅 [`CONTRIBUTING.md`](CONTRIBUTING.md)。产品行为和存储安全保证记录在 [`docs/design.md`](docs/design.md)。

```powershell
cargo run
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --release
```

## 许可证与致谢

本程序采用 [GPL-3.0-only](LICENSE) 许可证。其中两个内置预设来自 Mirror's Edge 速通社区：

- [69% 存档](https://www.speedrun.com/me/resources/4gwtx)，作者 Toyro98
- [完成主线存档](https://www.speedrun.com/me/resources/62y3z)，作者 Phillotrax

内置存档资源的来源和再分发说明见 [`resources/built-in/NOTICE.md`](resources/built-in/NOTICE.md)。

本项目为非官方项目，与 Electronic Arts 或 DICE 无关联，也未获得其认可。
