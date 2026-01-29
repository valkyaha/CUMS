use cums_sekiro::{FsbBank, Encryption};

fn main() {
    let path = r"G:\SteamLibrary\steamapps\common\Dark Souls II Scholar of the First Sin\Game\sound\frpg2_ps100200.fsb";

    println!("Loading: {}", path);

    match FsbBank::load(path) {
        Ok(bank) => {
            println!("Success!");
            println!("  Samples: {}", bank.samples.len());
            println!("  Codec: {:?}", bank.codec);
            println!("  Encrypted: {}", bank.encryption != Encryption::None);
            for (i, s) in bank.samples.iter().take(5).enumerate() {
                println!("  Sample {}: {:?} - {}Hz {}ch", i, s.name, s.frequency, s.channels);
            }
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}
