use super::out::*;
use std::process::exit;

pub(crate) async fn login() {
    let client = crate::cmd::client::CLIENT_CELL.get().unwrap();
    if !client.cookie.is_empty() {
        let confirm = dialoguer::Confirm::new()
            .with_prompt("检测到已有登录信息，是否覆盖？")
            .interact()
            .unwrap_or(false);
        if !confirm {
            warn("取消登录");
            return;
        }
    }
    let client = crate::cmd::client::CLIENT_CELL.get().unwrap();
    let url = client.web_login_qr_url().await.expect("获取登录二维码失败");
    if let Err(e) = qr2term::print_qr(url.url.as_str()) {
        warn(format!("生成二维码失败: {}", e).as_str());
    }
    info(format!("登录二维码链接: {}", url.url).as_str());
    info("请使用B站APP扫码登录，扫码后请等待几秒钟...");
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let verify = client
            .web_login_qr_verify(&url.qrcode_key)
            .await
            .expect("登录请求失败");
        // 86101: 二维码未扫描
        // 86090: 已扫描未确认
        // 86038: 二维码已失效
        if verify.code == 86101 || verify.code == 86090 {
            continue;
        }
        if verify.code == 0 {
            crate::cmd::client::store_login(&verify);
            break;
        }
        error(format!("登录失败: {} ({})", verify.message, verify.code).as_str());
        exit(1);
    }
    success("登录成功")
}
