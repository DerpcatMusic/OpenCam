fn main() {
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-changed=vendor/unity-capture");
    match std::env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
        "windows" => {
            cc::Build::new()
                .cpp(true)
                .std("c++17")
                .file("native/windows.cpp")
                .compile("opencam_camera");
            println!("cargo:rustc-link-lib=advapi32");
        }
        "macos" => {
            cc::Build::new()
                .cpp(true)
                .std("c++17")
                .flag("-fobjc-arc")
                .file("native/macos.mm")
                .compile("opencam_camera");
            for framework in ["Foundation", "CoreMedia", "CoreMediaIO", "CoreVideo"] {
                println!("cargo:rustc-link-lib=framework={framework}");
            }
        }
        _ => {}
    }
}
