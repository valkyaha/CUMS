use cums_sekiro::{FsbBank, Encryption};
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let orig_path = r"G:\SteamLibrary\steamapps\common\Sekiro\sound\main.fsb";
    let mod_path = r"G:\SteamLibrary\steamapps\common\Sekiro\mods\sound\main.fsb";

    println!("=== Comparing FSB files ===\n");

    let orig_bytes = fs::read(orig_path)?;
    let mod_bytes = fs::read(mod_path)?;

    println!("Original size: {} bytes", orig_bytes.len());
    println!("Modified size: {} bytes", mod_bytes.len());
    println!("Difference: {} bytes\n", mod_bytes.len() as i64 - orig_bytes.len() as i64);

    println!("Original first 4 bytes: {:02X} {:02X} {:02X} {:02X} ({})",
        orig_bytes[0], orig_bytes[1], orig_bytes[2], orig_bytes[3],
        if &orig_bytes[0..4] == b"FSB5" { "FSB5 - NOT encrypted" } else { "encrypted" });
    println!("Modified first 4 bytes: {:02X} {:02X} {:02X} {:02X} ({})\n",
        mod_bytes[0], mod_bytes[1], mod_bytes[2], mod_bytes[3],
        if &mod_bytes[0..4] == b"FSB5" { "FSB5 - NOT encrypted" } else { "encrypted" });

    println!("Loading original...");
    let orig = FsbBank::load(orig_path)?;
    println!("  Version: {:?}", orig.version);
    println!("  Samples: {}", orig.samples.len());
    println!("  Codec: {:?}", orig.codec);
    println!("  Encrypted: {}", orig.encryption != Encryption::None);

    println!("\nLoading modified...");
    let modded = FsbBank::load(mod_path)?;
    println!("  Version: {:?}", modded.version);
    println!("  Samples: {}", modded.samples.len());
    println!("  Codec: {:?}", modded.codec);
    println!("  Encrypted: {}", modded.encryption != Encryption::None);

    if orig.samples.len() != modded.samples.len() {
        println!("\n!!! SAMPLE COUNT MISMATCH !!!");
        println!("Original: {} samples", orig.samples.len());
        println!("Modified: {} samples", modded.samples.len());
    }

    println!("\n=== Sample Comparison (first 5) ===");
    for i in 0..5.min(orig.samples.len()) {
        let os = &orig.samples[i];
        let ms = &modded.samples[i];

        println!("\nSample {}:", i);
        println!("  Name: {:?} vs {:?}", os.name, ms.name);
        println!("  Freq: {} vs {}", os.frequency, ms.frequency);
        println!("  Channels: {} vs {}", os.channels, ms.channels);
        println!("  Samples: {} vs {}", os.samples, ms.samples);
        println!("  Data offset: {} vs {}", os.data_offset, ms.data_offset);
        println!("  Data size: {} vs {}", os.data_size, ms.data_size);
        println!("  Vorbis CRC: {:?} vs {:?}", os.vorbis_crc, ms.vorbis_crc);
    }

    println!("\n=== All Vorbis CRCs in ORIGINAL file ===");
    let mut orig_crcs: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for s in &orig.samples {
        if let Some(crc) = s.vorbis_crc {
            orig_crcs.insert(crc);
        }
    }
    for crc in &orig_crcs {
        println!("  0x{:08X}", crc);
    }

    println!("\n=== All Vorbis CRCs in MODIFIED file ===");
    let mut unique_crcs: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for s in &modded.samples {
        if let Some(crc) = s.vorbis_crc {
            unique_crcs.insert(crc);
        }
    }
    for crc in &unique_crcs {
        println!("  0x{:08X}", crc);
    }

    println!("\n=== NEW CRCs in modified (not in original) ===");
    for crc in &unique_crcs {
        if !orig_crcs.contains(crc) {
            println!("  0x{:08X} - NEW!", crc);
            for (i, s) in modded.samples.iter().enumerate() {
                if s.vorbis_crc == Some(*crc) {
                    println!("    -> Sample {}: {:?} ({}Hz {}ch)", i, s.name, s.frequency, s.channels);
                }
            }
        }
    }

    println!("\n=== CRCs REMOVED from original ===");
    for crc in &orig_crcs {
        if !unique_crcs.contains(crc) {
            println!("  0x{:08X} - REMOVED", crc);
            for (i, s) in orig.samples.iter().enumerate() {
                if s.vorbis_crc == Some(*crc) {
                    println!("    -> Was Sample {}: {:?} ({}Hz {}ch)", i, s.name, s.frequency, s.channels);
                }
            }
        }
    }

    println!("\n=== Sample 455 comparison ===");
    let os = &orig.samples[455];
    let ms = &modded.samples[455];
    println!("Original:");
    println!("  Name: {:?}", os.name);
    println!("  Freq: {}", os.frequency);
    println!("  Channels: {}", os.channels);
    println!("  Samples: {}", os.samples);
    println!("  Data size: {}", os.data_size);
    println!("  Vorbis CRC: 0x{:08X}", os.vorbis_crc.unwrap_or(0));
    println!("Modified:");
    println!("  Name: {:?}", ms.name);
    println!("  Freq: {}", ms.frequency);
    println!("  Channels: {}", ms.channels);
    println!("  Samples: {}", ms.samples);
    println!("  Data size: {}", ms.data_size);
    println!("  Vorbis CRC: 0x{:08X}", ms.vorbis_crc.unwrap_or(0));

    println!("\n=== All changed samples ===");
    for i in 0..orig.samples.len().min(modded.samples.len()) {
        let os = &orig.samples[i];
        let ms = &modded.samples[i];
        if os.vorbis_crc != ms.vorbis_crc || os.data_size != ms.data_size || os.samples != ms.samples {
            println!("Sample {} ({:?}):", i, ms.name);
            println!("  CRC: 0x{:08X} -> 0x{:08X}", os.vorbis_crc.unwrap_or(0), ms.vorbis_crc.unwrap_or(0));
            println!("  Size: {} -> {}", os.data_size, ms.data_size);
            println!("  Samples: {} -> {}", os.samples, ms.samples);
        }
    }

    Ok(())
}
