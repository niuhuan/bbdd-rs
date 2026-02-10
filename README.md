bbdd
====

bilibili download develop

一个用于下载bilibili视频的RUST库, 以及cli, BBDown的RUST版本

## 安装cli使用

```
# 安装命令行
cargo install bbdd --features cli,http2
# 安装命令行, 使用rustls
cargo install --path . --features=cli,http2,rustls-tls --no-default-features
# 安装命令行并启用rsmpeg支持(编译安装了ffmpeg并设置了FFMPEG_PKG_CONFIG_PATH)
cargo install bbdd --features cli,http2,rsmpeg
# 安装命令行并启用rsmpeg支持(设置了VCPKG_ROOT环境变量, 并且已经在VCPKG安装了ffmpeg)
cargo install bbdd --features=cli,http2,ffmpeg7_1,link_vcpkg_ffmpeg
```

```shell
# 第一次使用先登录
./bbdd login
# 下载视频
./bbdd <bilibili视频url/BV号/SS号/EP号>
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
    - [x] BV
    - [x] SS, EP
- 功能
    - [x] 断点续传
    - [ ] ~~多线程、多分段下载~~ (提升约15%, 但会增加风险, 不考虑实现)
    - [ ] 选择清晰度
- [ ] 拓展
    - [ ] 下载字幕
    - [ ] 下载封面

## 引入依赖使用

可以参考单元测试代码或者cli的代码

#### CLI

[src/cmd/mod.rs](src/cmd/mod.rs)

#### 单元测试

- 创建客户端 : [src/tests.rs](src/tests.rs)  #bbdd
- 登录 : [src/auth/web.rs](src/auth/web.rs)  #test_web_log_qr_url
- 解析视频url : [src/parse.rs](ssrc/parse.rs)  #test_parse_url
- 下载视频 : [src/download.rs](src/download.rs)  #test_bili_download

#### 特性

- 下载
  - [x] BV
  - [x] SS, EP
