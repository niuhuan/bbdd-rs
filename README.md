bbdd
====

bilibili download develop

一个用于下载bilibili视频的依赖库以及cli

## 引入依赖使用

创建客户端 : [src/tests.rs](src/tests.rs)  #bbdd
登录 : [src/auth/web.rs](src/auth/web.rs)  #test_web_log_qr_url
解析视频url : [src/parse.rs](ssrc/parse.rs)  #test_parse_url
下载视频 : [src/download.rs](src/download.rs)  #test_bili_download

## 安装cli使用 (未完成)

```
cargo install bbdd --features cli
```
