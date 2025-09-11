use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();

    // 只在启用 rsmpeg feature 时处理 FFmpeg 链接
    if cfg!(feature = "rsmpeg") && target.contains("apple-darwin") {
        let vcpkg_root = env::var("VCPKG_ROOT").unwrap_or_else(|_| {
            // 如果 VCPKG_ROOT 未设置，尝试从常见路径查找
            let possible_paths = [
                "./vcpkg",
                "../vcpkg",
                "../../vcpkg",
                "/usr/local/vcpkg",
            ];

            for path in possible_paths {
                if std::path::Path::new(path).exists() {
                    return path.to_string();
                }
            }

            panic!("VCPKG_ROOT not set and vcpkg not found in common locations");
        });

        let lib_dir = if target.contains("aarch64") {
            format!("{}/installed/arm64-osx/lib", vcpkg_root)
        } else {
            format!("{}/installed/x64-osx/lib", vcpkg_root)
        };

        // 检查库目录是否存在
        if !std::path::Path::new(&lib_dir).exists() {
            panic!("FFmpeg library directory not found: {}", lib_dir);
        }

        println!("cargo:rustc-link-search=native={}", lib_dir);

        // 链接 FFmpeg 静态库
        println!("cargo:rustc-link-lib=static=avdevice");
        println!("cargo:rustc-link-lib=static=avfilter");
        println!("cargo:rustc-link-lib=static=avformat");
        println!("cargo:rustc-link-lib=static=avcodec");
        println!("cargo:rustc-link-lib=static=swresample");
        println!("cargo:rustc-link-lib=static=swscale");
        println!("cargo:rustc-link-lib=static=avutil");

        // macOS 必需的框架
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=CoreVideo");
        println!("cargo:rustc-link-lib=framework=VideoToolbox");
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=Foundation");

        // 其他可能需要的系统库
        println!("cargo:rustc-link-lib=framework=OpenGL");
        println!("cargo:rustc-link-lib=framework=QuartzCore");

        println!("cargo:rerun-if-changed={}", lib_dir);
        println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
    }
}
