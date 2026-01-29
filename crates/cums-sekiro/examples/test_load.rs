use cums_sekiro::{rebuild_ogg, Encryption, FsbBank};
use std::io::Write;

fn main() {
    let path = "examples/sekiro/original/main.fsb";

    println!("Loading: {}", path);

    match FsbBank::load(path) {
        Ok(bank) => {
            println!("\nFSB Bank loaded successfully!");
            println!("  Version: {:?}", bank.version);
            println!("  Codec: {:?}", bank.codec);
            println!("  Encrypted: {}", bank.encryption != Encryption::None);
            println!("  Samples: {}", bank.samples.len());

            println!("\nFirst 10 samples:");
            for (i, sample) in bank.samples.iter().take(10).enumerate() {
                println!(
                    "  [{}] {} - {}Hz {}ch, {} samples ({:.2}s), offset=0x{:X}, size={}",
                    i,
                    sample.name.as_deref().unwrap_or("<unnamed>"),
                    sample.frequency,
                    sample.channels,
                    sample.samples,
                    sample.samples as f64 / sample.frequency as f64,
                    sample.data_offset,
                    sample.data_size,
                );
                if let Some(crc) = sample.vorbis_crc {
                    println!("       Vorbis CRC: 0x{:08X}", crc);
                }
            }

            if !bank.samples.is_empty() {
                println!("\nAttempting to extract first sample as OGG...");
                let sample = &bank.samples[0];
                match rebuild_ogg(&bank, sample) {
                    Ok(ogg_data) => {
                        println!("  Success! OGG size: {} bytes", ogg_data.len());

                        let output_path = "test_output.ogg";
                        match std::fs::File::create(output_path) {
                            Ok(mut file) => {
                                if file.write_all(&ogg_data).is_ok() {
                                    println!("  Saved to: {}", output_path);
                                }
                            }
                            Err(e) => println!("  Failed to save: {}", e),
                        }
                    }
                    Err(e) => println!("  Failed to rebuild OGG: {}", e),
                }
            }
        }
        Err(e) => {
            println!("Error loading FSB: {}", e);
        }
    }
}
