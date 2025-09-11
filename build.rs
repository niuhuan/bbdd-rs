use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();

    // 只在启用 rsmpeg feature 时处理 FFmpeg 链接
    if cfg!(feature = "rsmpeg") && target.contains("apple-darwin") {

        let vcpkg_root = env::var("VCPKG_ROOT").unwrap_or_else(|_| {
            "".to_string()
        });

        if !vcpkg_root.is_empty() {
            let lib_dir = if target.contains("aarch64") {
                format!("{}/installed/arm64-osx/lib", vcpkg_root)
            } else {
                format!("{}/installed/x64-osx/lib", vcpkg_root)
            };
    
            if std::path::Path::new(&lib_dir).exists() {
                println!("cargo:rustc-link-search=native={}", lib_dir);
            }

            println!("cargo:rerun-if-changed={}", lib_dir);
        }
        println!("cargo:rerun-if-env-changed=VCPKG_ROOT");


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

    }

    // 只在启用 rsmpeg feature 时处理 FFmpeg 链接
    if cfg!(feature = "rsmpeg") && target.contains("windows") {
        // mfplat mfreadwrite mfuuid propsys
        println!("cargo:rustc-link-lib=mfuuid");
        println!("cargo:rustc-link-lib=mfplat");
        println!("cargo:rustc-link-lib=mf");
        println!("cargo:rustc-link-lib=mfreadwrite");
    }
}
