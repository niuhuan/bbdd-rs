bbdd
====

bilibili download develop

RUST 一个用于下载bilibili视频的依赖库以及cli

## 引入依赖使用

- 创建客户端 : [src/tests.rs](src/tests.rs)  #bbdd
- 登录 : [src/auth/web.rs](src/auth/web.rs)  #test_web_log_qr_url
- 解析视频url : [src/parse.rs](ssrc/parse.rs)  #test_parse_url
- 下载视频 : [src/download.rs](src/download.rs)  #test_bili_download

- 下载
  - [x] BV
  - [x] SS, EP

## 安装cli使用

```
# 安装命令行
cargo install bbdd --features cli
# 安装命令行并启用rsmpeg支持, 将ffmpeg编译进二进制文件
cargo install bbdd --features cli,rsmpeg
```

```shell
# 第一次使用先登录
./bbdd login
# 下载视频
./bbdd <bilibili视频url/BV号>
```

```shell 
./bbdd --help

一个BILIBILI视频下载工具

Usage: bbdd [OPTIONS] [url] [COMMAND]

Commands:
  login  登录BILIBILI账号
  help   Print this message or the help of the given subcommand(s)

Arguments:
  [url]  

Options:
  -w, --workdir <DIR>     工作目录，默认为当前目录，目录必须存在才能使用
  -i, --interactive       遇到已经下载的文件时，进行提问是否覆盖 （默认不提问、不覆盖，直接跳过）
  -o, --overwrite         遇到已经下载的文件时，直接进行覆盖 （默认不覆盖，直接跳过）
  -c, --continue <CACHE>  下载中断时是否保留的缓存，再次下载时是否使用缓存，-o存在时此选项默认为false，其余时为true，缓存为.video和.audio结尾的文件
  -h, --help              Print help
  -V, --version           Print version
```

- 下载
  - [x] BV
  - [ ] SS, EP
- 功能
  - [x] 断点续传
  - [ ] 多线程下载
  - [ ] 选择清晰度
- [ ] 拓展
  - [ ] 下载字幕
  - [ ] 下载封面

