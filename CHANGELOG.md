# Changelog

## 0.4.0 — 2026-11-26

相对 0.3.1 是破坏性更新。

### 破坏性

- `BBDDError` 增加 `Io` 和 `Cancelled`。穷尽匹配这个错误的代码要补上这两支。
- `MediaListInfo` 增加 `cover`，`MediaListItem` 增加 `video_title`。按字段构造这两个结构的代码要补上新字段。
- Android 和 iOS 上如果开了 `rsmpeg` 或 `ffmpeg*`，却没开 `rsmpeg-build`，编译会失败。这两个目标没有系统 libav。
- 命令行现在默认下载封面、写入视频，然后删除临时图片。`--no-cover` 跳过，`--keep-cover` 保留图片。

### 新增

- `BBDD::new` 可以用 cookie 和 User-Agent 直接构造客户端。
- `BBDD::download_file` 写入文件，支持续传，并通过 `progress::Control` 报告进度和取消。
- 公开 `progress`、`select`。开了 `rsmpeg` 或 `rsmpeg-build` 时公开 `merge::merge_files`，封面作为 attached picture 写入。
- 新 feature `rsmpeg-build`：从源码静态编译 LGPL FFmpeg。`rsmpeg` 不再要求同时打开 `cli`。
- 命令行增加 `cover` 子命令，只保存封面。`-w/--workdir` 对子命令全局生效。
- 修掉 `__playinfo__` 正则：花括号未转义时会在解析时 panic。编译失败时改为返回空。
