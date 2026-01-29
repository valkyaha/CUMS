use cums_sekiro::FsbBank;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let orig_path = r"G:\SteamLibrary\steamapps\common\Sekiro\sound\main.fsb";

    println!("Loading original FSB...");
    let bank = FsbBank::load(orig_path)?;

    println!("Total samples: {}\n", bank.samples.len());

    let mut has_vorbis_crc = 0;
    let mut has_seek_table = 0;
    let mut has_loop = 0;
    let mut total_seek_entries = 0;

    for sample in &bank.samples {
        if sample.vorbis_crc.is_some() {
            has_vorbis_crc += 1;
        }
        if let Some(table) = &sample.vorbis_seek_table {
            has_seek_table += 1;
            total_seek_entries += table.len();
        }
        if sample.loop_start.is_some() {
            has_loop += 1;
        }
    }

    println!("Samples with Vorbis CRC: {}", has_vorbis_crc);
    println!("Samples with seek table: {}", has_seek_table);
    println!("Samples with loop points: {}", has_loop);
    println!("Total seek table entries: {}", total_seek_entries);

    let mut expected_size = bank.samples.len() * 8;

    for sample in &bank.samples {
        if sample.vorbis_crc.is_some() {
            expected_size += 4;
            expected_size += 4;
            if let Some(table) = &sample.vorbis_seek_table {
                expected_size += table.len() * 4;
            }
        }
        if sample.loop_start.is_some() {
            expected_size += 4;
            expected_size += 8;
        }
    }

    println!("\nExpected sample headers size: {} bytes", expected_size);

    println!("\n=== First 10 samples with chunks ===");
    let mut shown = 0;
    for (i, sample) in bank.samples.iter().enumerate() {
        if sample.vorbis_crc.is_some() || sample.loop_start.is_some() {
            println!("Sample {} {:?}:", i, sample.name);
            println!("  Vorbis CRC: {:?}", sample.vorbis_crc.map(|c| format!("0x{:08X}", c)));
            println!("  Seek table: {} entries", sample.vorbis_seek_table.as_ref().map(|t| t.len()).unwrap_or(0));
            println!("  Loop: {:?} - {:?}", sample.loop_start, sample.loop_end);
            shown += 1;
            if shown >= 10 {
                break;
            }
        }
    }

    Ok(())
}
