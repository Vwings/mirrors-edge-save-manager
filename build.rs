fn main() {
    println!("cargo:rerun-if-changed=ui/app-window.slint");
    println!("cargo:rerun-if-changed=translations");
    println!("cargo:rerun-if-changed=resources/app-icon.png");
    println!("cargo:rerun-if-changed=resources/app-icon.ico");
    println!("cargo:rerun-if-changed=resources/header-logo.png");
    println!("cargo:rerun-if-changed=resources/window-icon.png");
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("translations")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app-window.slint", config)
        .expect("failed to compile Slint UI");

    #[cfg(windows)]
    {
        use winresource::{VersionInfo, WindowsResource};

        let version = package_version();
        WindowsResource::new()
            .set_icon("resources/app-icon.ico")
            .set("FileDescription", "Mirror’s Edge Save Manager")
            .set("ProductName", "Mirror’s Edge Save Manager")
            .set("InternalName", "mirrors-edge-save-manager")
            .set("OriginalFilename", "mirrors-edge-save-manager.exe")
            .set(
                "Comments",
                "Unofficial save preset and recovery manager for Mirror’s Edge",
            )
            .set_version_info(VersionInfo::FILEVERSION, version)
            .set_version_info(VersionInfo::PRODUCTVERSION, version)
            .compile()
            .expect("failed to compile Windows application resources");
    }
}

#[cfg(windows)]
fn package_version() -> u64 {
    let mut components = env!("CARGO_PKG_VERSION")
        .split('.')
        .take(4)
        .map(|component| component.parse::<u16>().unwrap_or(0));

    let major = components.next().unwrap_or(0) as u64;
    let minor = components.next().unwrap_or(0) as u64;
    let patch = components.next().unwrap_or(0) as u64;
    let revision = components.next().unwrap_or(0) as u64;
    (major << 48) | (minor << 32) | (patch << 16) | revision
}
