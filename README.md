bbdd
====

bilibili download develop

一个用于下载bilibili视频的依赖库以及cli

## 引入依赖使用

- 创建客户端 : [src/tests.rs](src/tests.rs)  #bbdd
- 登录 : [src/auth/web.rs](src/auth/web.rs)  #test_web_log_qr_url
- 解析视频url : [src/parse.rs](ssrc/parse.rs)  #test_parse_url
- 下载视频 : [src/download.rs](src/download.rs)  #test_bili_download

## 安装cli使用

- 下载
  - [x] BV
  - [ ] SS, EP
- 功能
  - [ ] 断点续传
  - [ ] 多线程下载
  - [ ] 选择清晰度
- [ ] 拓展
  - [ ] 下载字幕
  - [ ] 下载封面

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

