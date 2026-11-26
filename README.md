bbdd
====

bilibili download develop

一个用于下载bilibili视频的RUST库, 以及cli, BBDown的RUST版本

## 安装环境

Rust stable。TLS 必须开 `native-tls`（默认）或 `rustls` 其中之一。要进程内合并时再准备 FFmpeg；不开 `rsmpeg` / `ffmpeg*` / `rsmpeg-build` 时，命令行改为调用外部 `ffmpeg`。

| 平台 | 依赖 | 环境 |
| --- | --- | --- |
| macOS（Apple Silicon，只编本机） | vcpkg `ffmpeg:arm64-osx`，或本机 pkg-config 能找到的 libav | `VCPKG_ROOT`，`PKG_CONFIG_PATH=$VCPKG_ROOT/installed/arm64-osx/lib/pkgconfig` |
| macOS（通用二进制，arm64 + x86_64） | 上面再加 `ffmpeg:x64-osx` | 再加下面的交叉变量。变量名里的 `-` 要写成 `_`，shell 不能导出带连字符的名字 |
| Linux | 发行版的 `ffmpeg` / `libav*-dev`，pkg-config 能找到 | 不需要 vcpkg |
| Windows | vcpkg `ffmpeg:x64-windows`，或外部 `ffmpeg.exe` | `VCPKG_ROOT`。`link_vcpkg_ffmpeg` 在 MSVC 上由构建脚本读 vcpkg |
| Android / iOS | 不链接系统 libav。混流必须开 `rsmpeg-build`，从源码编 LGPL FFmpeg | Android NDK、Xcode。不要开单独的 `rsmpeg` 或 `ffmpeg*` |

macOS 通用二进制：

```shell
export VCPKG_ROOT="$HOME/Runtimes/vcpkg"
export PKG_CONFIG_PATH="$VCPKG_ROOT/installed/arm64-osx/lib/pkgconfig"
export PKG_CONFIG_ALLOW_CROSS_x86_64_apple_darwin=1
export PKG_CONFIG_PATH_x86_64_apple_darwin="$VCPKG_ROOT/installed/x64-osx/lib/pkgconfig"
```

`ffmpeg-sys-next` 在 macOS 上不读 `VCPKG_ROOT`，只认 `PKG_CONFIG_PATH`。`x86_64-apple-darwin` 是交叉编译，还要目标自己的 `PKG_CONFIG_PATH_x86_64_apple_darwin`。

## 安装 cli

在仓库里：

```shell
# 外部 ffmpeg 合并
cargo install --path . --features cli,http2
# rustls，外部 ffmpeg
cargo install --path . --features=cli,http2,rustls --no-default-features
# 进程内混流。pkg-config 必须已经能找到 libav
cargo install --path . --features=cli,http2,rustls,ffmpeg9,link_vcpkg_ffmpeg --no-default-features
```

`ffmpeg6` 到 `ffmpeg9` 都只是 `rsmpeg` 的别名，不绑定某一版 FFmpeg API。`link_vcpkg_ffmpeg` 在此之上静态链接 vcpkg 的库。


```shell
# 第一次使用先登录
./bbdd login
./bbdd whoami
# 下载视频。封面默认写进 mp4 后删除本地图片
./bbdd <bilibili视频url/BV号/SS号/EP号>
./bbdd --keep-cover <url>
./bbdd --no-cover <url>
# 只保存封面
./bbdd cover <url>
```

```text
./bbdd --help

一个BILIBILI视频下载工具

Usage: bbdd [OPTIONS] [url] [COMMAND]

Commands:
  login  登录BILIBILI账号
  help   Print this message or the help of the given subcommand(s)

Arguments:
  [url]

Options:
  -q, --quality <QUALITY>  视频清晰度，默认为最高清晰度, 参数为数字。 超过48可能需要大会员用户。127(8K 超高清), 126(杜比视界), 125(HDR 真彩), 120(4K 超清), 116(1080P 高帧率), 112(1080P 高码率), 100(智能修复), 80(1080P 高清), 74(720P 高帧率), 64(720P 高清), 48(720P 高清), 32(480P 清晰), 16(360P 流畅), 6(240P 流畅), 5(144P 流畅)
  -p, --select-page <PAGE> 选择分P或集数，例如 8、1,2、3-5、ALL、LAST，默认 ALL
  -w, --workdir <DIR>      工作目录，默认为当前目录，目录必须存在才能使用
  -i, --interactive        遇到已经下载的文件时，进行提问是否覆盖 （默认不提问、不覆盖，直接跳过）
  -o, --overwrite          遇到已经下载的文件时，直接进行覆盖 （默认不覆盖，直接跳过）
  -c, --continue <CACHE>   下载中断时是否保留的缓存，再次下载时是否使用缓存，-o存在时此选项默认为false，其余时为true，缓存为.video.*和.audio.*结尾的文件
      --debug              启用调试模式，输出更多日志
  -h, --help               Print help
  -V, --version            Print version
```
#### 特性

- 下载
    - [x] BV，包括多分P
    - [x] SS, EP
    - [x] 课堂
    - [x] 合集、系列（含新版空间 `/lists/{id}` 链接）
    - [x] 收藏夹
- 功能
    - [x] 断点续传
    - [ ] ~~多线程、多分段下载~~ (提升约15%, 但会增加风险, 不考虑实现)
    - [x] 选择清晰度
    - [x] 选择分P / 集数 (`-p`)
    - [x] 下载失败自动重试
- [x] 拓展
    - [x] 下载字幕（登录后，保存为 SRT）
    - [x] 下载封面（默认写入 mp4 后删除图片；`--keep-cover` 保留，`--no-cover` 跳过。`cover` 子命令只保存图片）

## Features

必须开 `native-tls`（默认）或 `rustls` 其中之一。

| feature | 作用 |
| --- | --- |
| `cli` | 命令行。不包含混流。 |
| `rsmpeg` | 公开 `bbdd::merge::merge_files`，链接本机或 vcpkg 的 libav。桌面用。 |
| `ffmpeg6` `ffmpeg7` `ffmpeg7_1` `ffmpeg8` `ffmpeg9` | 旧安装命令的别名，都等于 `rsmpeg`。不绑定某一版 API。 |
| `link_vcpkg_ffmpeg` | 在 `rsmpeg` 上改为静态链接 vcpkg 的 FFmpeg。 |
| `rsmpeg-build` | 从源码静态编译 LGPL FFmpeg 并公开 `merge_files`。Android / iOS 要混流时必须开这个。 |
| `http2` / `http3` | reqwest 的协议。`http3` 会带上 `rustls`。 |

没开 `rsmpeg`、`ffmpeg*`、`rsmpeg-build` 时，库只提供分开的音视频下载，没有合并函数。命令行在这种情况下调用外部 `ffmpeg`，取消时杀掉这个进程。开了上述任一 feature 时，命令行改为调用 `bbdd::merge`。

下载和合并的进度、取消都在库里。`BBDD::download_file` 和 `merge::merge_files` 接收 `progress::Control`：进度大约每 100ms 推一条 `Update`，`CancellationToken` 置位后当前读块或合并包会停。命令行只消费这些事件来画进度条，并把 Ctrl-C 接到这个 token。进度条不在库里。

Android 和 iOS 上如果开了 `rsmpeg` 或 `ffmpeg*` 却没开 `rsmpeg-build`，编译会失败。这两个目标没有系统 libav。

## 作为依赖使用

桌面库链接本机或 vcpkg 的 libav。手机库自己编 FFmpeg。

```toml
[dependencies]
# macOS / Linux / Windows
bbdd = { path = "../bbdd-rs", default-features = false, features = ["rustls", "http2", "rsmpeg"] }

# Android / iOS
# bbdd = { path = "../bbdd-rs", default-features = false, features = ["rustls", "http2", "rsmpeg-build"] }
```

```shell
cargo build --lib --no-default-features --features rustls,http2,rsmpeg
cargo build --lib --no-default-features --features rustls,http2,rsmpeg-build --target aarch64-linux-android
cargo build --lib --no-default-features --features rustls,http2,rsmpeg-build --target aarch64-apple-ios
```

静态库只记录对 `libav*` 的引用。最终链接还要把对应架构的 `libavcodec`、`libavformat`、`libavutil`、`libswscale`、`libswresample`、`libavfilter`、`libavdevice` 加进去。Flutter 应用这一步写在 `bbdd/macos/Podfile`。

可以参考单元测试代码或者 cli 的代码。

#### CLI

[src/cmd/mod.rs](src/cmd/mod.rs)

#### 单元测试

- 创建客户端 : [src/tests.rs](src/tests.rs)  #bbdd
- 登录 : [src/auth/web.rs](src/auth/web.rs)  #test_web_log_qr_url
- 解析视频url : [src/parse.rs](src/parse.rs)  #test_parse_bv
- 下载视频 : [src/download.rs](src/download.rs)  #test_bili_download

#### 特性

库接口覆盖普通视频、番剧、课堂、合集、系列和收藏夹。命令行负责下载、字幕和封面。

## 安装 vcpkg FFmpeg

```shell
git clone https://github.com/microsoft/vcpkg.git
cd vcpkg
./bootstrap-vcpkg.sh
# 只要本机架构时装一个 triplet。macOS 通用二进制两个都要。
./vcpkg install ffmpeg:arm64-osx
./vcpkg install ffmpeg:x64-osx
export VCPKG_ROOT="$(pwd)"
```

Windows 用 `ffmpeg:x64-windows`。Linux 用系统包即可，不必走 vcpkg。

