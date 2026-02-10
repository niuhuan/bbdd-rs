use super::out::{info, success, warn};

pub(crate) async fn whoami() {
    let client = crate::cmd::client::CLIENT_CELL.get().unwrap();
    if client.cookie.is_empty() {
        warn("当前未加载到 cookie（未登录或未保存登录信息）");
    }

    let nav = match client.web_nav().await {
        Ok(v) => v,
        Err(e) => {
            warn(format!("认证请求失败（将按未登录处理）: {}", super::format_bbdd_error(&e)).as_str());
            return;
        }
    };

    if !nav.is_login {
        warn("当前账号状态: 未登录");
        info("可运行 `bbdd login` 重新登录（或检查本地 web_login.json 是否有效）");
        print_cookie_hints(client.cookie.as_str());
        return;
    }

    success("当前账号状态: 已登录");
    info(format!("用户: {} (mid={})", nav.uname, nav.mid).as_str());
    if nav.vip.status == 1 {
        info(format!("会员: {} (type={})", nav.vip.label.text, nav.vip.vip_type).as_str());
    } else {
        info("会员: 否");
    }
    info(format!("余额: {}", nav.money).as_str());
    print_cookie_hints(client.cookie.as_str());
}

fn print_cookie_hints(cookie: &str) {
    if cookie.is_empty() {
        return;
    }
    let missing = [
        ("SESSDATA", "通常决定登录态/会员清晰度"),
        ("bili_jct", "部分接口可能需要"),
        ("DedeUserID", "账号标识"),
    ]
    .into_iter()
    .filter(|(k, _)| !cookie.contains(&format!("{k}=")))
    .collect::<Vec<_>>();
    if !missing.is_empty() {
        warn(
            format!(
                "cookie 可能不完整，缺少: {}",
                missing
                    .iter()
                    .map(|(k, why)| format!("{k}({why})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .as_str(),
        );
    }
}

