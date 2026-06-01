fn main() {
    // Lấy thư mục chứa dự án và tìm file librclone.a / rclone.lib ở thư mục cha hoặc ông
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = std::path::Path::new(&manifest_dir);
    if let Some(parent) = manifest_path.parent() {
        println!("cargo:rustc-link-search=native={}", parent.display());
        if let Some(grandparent) = parent.parent() {
            println!("cargo:rustc-link-search=native={}", grandparent.display());
        }
    }

    // Yêu cầu liên kết tĩnh với rclone (tìm file librclone.a hoặc rclone.lib)
    println!("cargo:rustc-link-lib=static=rclone");

    let target = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target == "windows" {
        println!("cargo:rustc-link-lib=dylib=ws2_32");
        println!("cargo:rustc-link-lib=dylib=userenv");
        println!("cargo:rustc-link-lib=dylib=bcrypt");
        println!("cargo:rustc-link-lib=dylib=iphlpapi");
        println!("cargo:rustc-link-lib=dylib=winmm");
        println!("cargo:rustc-link-lib=dylib=shell32");
        if target_env == "msvc" {
            println!("cargo:rustc-link-lib=legacy_stdio_definitions");
        }
    } else {
        println!("cargo:rustc-link-lib=dylib=pthread");
        println!("cargo:rustc-link-lib=dylib=dl");

        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if target_os == "macos" {
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
            println!("cargo:rustc-link-lib=framework=Security");
            println!("cargo:rustc-link-lib=framework=SystemConfiguration");
        }
    }
}
