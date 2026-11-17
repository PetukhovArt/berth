fn main() {
    #[cfg(feature = "desktop")]
    {
        let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
        if windows && std::env::var_os("CARGO_FEATURE_VOICE").is_some() {
            println!("cargo:rustc-link-arg-bin=berth-desktop=/DELAYLOAD:sherpa-onnx-c-api.dll");
            println!("cargo:rustc-link-arg-bin=berth-desktop=delayimp.lib");
        }
        tauri_build::build()
    }
}
