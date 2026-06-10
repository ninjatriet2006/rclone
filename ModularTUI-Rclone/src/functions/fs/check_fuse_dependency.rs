pub fn check_fuse_dependency() -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        which::which("fusermount3").is_ok() || which::which("fusermount").is_ok()
    }
    #[cfg(target_os = "macos")]
    {
        std::path::Path::new("/Library/Filesystems/macfuse.fs").exists()
            || std::path::Path::new("/Library/Filesystems/osxfuse.fs").exists()
            || which::which("fuse-t").is_ok()
    }
    #[cfg(windows)]
    {
        std::env::var("WinfspDir").is_ok()
            || std::path::Path::new("C:\\Program Files (x86)\\WinFsp").exists()
            || std::path::Path::new("C:\\Program Files\\WinFsp").exists()
    }
}
