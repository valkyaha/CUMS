use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use cums_sekiro::{FsbBank, Codec, Version, Encryption, rebuild_ogg, extract_mp3};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} <fsb_file> [output_dir]", args[0]);
        println!();
        println!("Extracts audio samples from FSB4 or FSB5 files.");
        println!();
        println!("FSB4 (Dark Souls 1/2): Extracts MP3 audio");
        println!("FSB5 (Dark Souls 3/Sekiro): Extracts Vorbis/OGG audio");
        return Ok(());
    }

    let fsb_path = &args[1];
    let output_dir = if args.len() > 2 { &args[2] } else { "." };

    println!("Loading: {}", fsb_path);

    let bank = FsbBank::load(fsb_path)?;

    println!("Version: {:?}", bank.version);
    println!("Codec: {:?}", bank.codec);
    println!("Samples: {}", bank.samples.len());
    println!("Encrypted: {}", bank.encryption != Encryption::None);

    fs::create_dir_all(output_dir)?;

    for (i, sample) in bank.samples.iter().enumerate() {
        let default_name = format!("sample_{:04}", i);
        let name = sample.name.as_deref().unwrap_or(&default_name);

        let (audio_data, ext) = match (bank.version, bank.codec) {
            (Version::Fsb5, Codec::Vorbis) => {
                match rebuild_ogg(&bank, sample) {
                    Ok(ogg) => (ogg, "ogg"),
                    Err(e) => {
                        println!("  Warning: Failed to rebuild OGG for {}: {}", name, e);
                        (bank.sample_data(i)?.to_vec(), "vorbis_raw")
                    }
                }
            }
            (_, Codec::Mpeg) => {
                (extract_mp3(&bank, sample)?, "mp3")
            }
            _ => {
                (bank.sample_data(i)?.to_vec(), "bin")
            }
        };

        let output_path = Path::new(output_dir).join(format!("{}.{}", name, ext));

        println!("  [{}/{}] {} - {} Hz, {} ch, {:.2}s -> {:?}",
            i + 1, bank.samples.len(),
            name,
            sample.frequency,
            sample.channels,
            sample.samples as f64 / sample.frequency as f64,
            output_path
        );

        let mut file = File::create(&output_path)?;
        file.write_all(&audio_data)?;
    }

    println!("Done!");
    Ok(())
}
