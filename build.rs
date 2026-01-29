// Build script for platform-specific configurations

fn main() {
    // Windows-specific settings
    #[cfg(target_os = "windows")]
    {
        // Enable Windows subsystem for GUI (when we add it)
        // println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");

        // Link required Windows libraries
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=shell32");
    }

    // Linux-specific settings
    #[cfg(target_os = "linux")]
    {
        // Link against system audio libraries if needed
        // println!("cargo:rustc-link-lib=asound");
    }

    // Print rebuild triggers
    println!("cargo:rerun-if-changed=build.rs");
}
