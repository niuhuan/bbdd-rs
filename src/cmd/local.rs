use std::path::{Path, PathBuf};

/// 初始化配置文件目录
pub(crate) fn init_dir() -> String {
    let dir = cfg_local_dir();
    std::fs::create_dir_all(dir.as_str()).expect(
        format!("创建配置文件目录失败: {}", dir.as_str()).as_str(),
    );
    dir
}

pub(crate) fn join_paths<P: AsRef<Path>>(paths: Vec<P>) -> String {
    match paths.len() {
        0 => String::default(),
        _ => {
            let mut path: PathBuf = PathBuf::new();
            for x in paths {
                path = path.join(x);
            }
            return path.to_str().unwrap().to_string();
        }
    }
}

/// 取配置文件目录
#[cfg(target_os = "macos")]
pub(crate) fn cfg_local_dir() -> String {
    join_paths(vec![
        dirs::home_dir().unwrap().to_str().unwrap(),
        "Library",
        "Application Support",
        env!("CARGO_PKG_NAME"),
    ])
}

#[cfg(target_os = "windows")]
pub(crate) fn cfg_local_dir() -> String {
    join_paths(vec![
        dirs::home_dir().unwrap().to_str().unwrap(),
        "AppData",
        "Roaming",
        env!("CARGO_PKG_NAME"),
    ])
}

#[cfg(target_os = "linux")]
pub(crate) fn cfg_local_dir() -> String {
    join_paths(vec![
        dirs::home_dir().unwrap().to_str().unwrap(),
        format!(".{}", env!("CARGO_PKG_NAME")).as_str(),
    ])
}
