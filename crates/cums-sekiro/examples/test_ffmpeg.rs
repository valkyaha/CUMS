use std::path::PathBuf;
use std::process::Command;

fn find_ffmpeg() -> Option<PathBuf> {
    // First try PATH
    if Command::new("ffmpeg").arg("-version").output().is_ok() {
        println!("Found ffmpeg in PATH");
        return Some(PathBuf::from("ffmpeg"));
    }

    // Check WinGet packages location
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let winget_path = std::path::Path::new(&local_app_data)
            .join("Microsoft")
            .join("WinGet")
            .join("Packages");

        println!("Checking WinGet path: {:?}", winget_path);

        if winget_path.exists() {
            if let Ok(entries) = std::fs::read_dir(&winget_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    println!("  Checking: {:?}", path);
                    if path.is_dir() {
                        // Look for ffmpeg in subdirectories
                        let ffmpeg_candidates =
                            [path.join("bin").join("ffmpeg.exe"), path.join("ffmpeg.exe")];

                        // Also search one level deeper (for versioned folders)
                        if let Ok(sub_entries) = std::fs::read_dir(&path) {
                            for sub_entry in sub_entries.flatten() {
                                let sub_path = sub_entry.path();
                                if sub_path.is_dir() {
                                    let deep_ffmpeg = sub_path.join("bin").join("ffmpeg.exe");
                                    println!("    Checking deep: {:?}", deep_ffmpeg);
                                    if deep_ffmpeg.exists() {
                                        println!("    FOUND!");
                                        return Some(deep_ffmpeg);
                                    }
                                }
                            }
                        }

                        for candidate in &ffmpeg_candidates {
                            println!("    Checking: {:?}", candidate);
                            if candidate.exists() {
                                println!("    FOUND!");
                                return Some(candidate.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    println!("FFmpeg not found!");
    None
}

fn main() {
    println!("Testing FFmpeg finder...\n");

    match find_ffmpeg() {
        Some(path) => {
            println!("\nFFmpeg found at: {:?}", path);

            // Test it
            let output = Command::new(&path).arg("-version").output();

            match output {
                Ok(o) => {
                    println!("FFmpeg version output:");
                    println!(
                        "{}",
                        String::from_utf8_lossy(&o.stdout)
                            .lines()
                            .next()
                            .unwrap_or("")
                    );
                }
                Err(e) => println!("Failed to run ffmpeg: {}", e),
            }
        }
        None => println!("\nFFmpeg NOT found!"),
    }
}
