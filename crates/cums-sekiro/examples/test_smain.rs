use cums_sekiro::{FsbBank, FSB_KEY};

fn fsbdec(t: u8) -> u8 {
    let t = t as u32;
    ((((((((t & 64) | (t >> 2)) >> 2) | (t & 32)) >> 2) |
        (t & 16)) >> 1) | (((((((t & 2) | (t << 2)) << 2) |
        (t & 4)) << 2) | (t & 8)) << 1)) as u8
}

fn main() {
    println!("=== Round-trip test ===\n");

    let bank = FsbBank::load(r"G:\SteamLibrary\steamapps\common\Sekiro\sound\smain.fsb").unwrap();
    bank.save(r"G:\SteamLibrary\steamapps\common\Sekiro\mods\sound\smain_test.fsb", true).unwrap();
    println!("Saved test file\n");

    let orig = std::fs::read(r"G:\SteamLibrary\steamapps\common\Sekiro\sound\smain.fsb").unwrap();
    let test = std::fs::read(r"G:\SteamLibrary\steamapps\common\Sekiro\mods\sound\smain_test.fsb").unwrap();

    println!("Original: {} bytes", orig.len());
    println!("Test:     {} bytes", test.len());

    if orig.len() != test.len() {
        println!("Size mismatch!");
        return;
    }

    let mut diffs = 0;
    for (_i, (a, b)) in orig.iter().zip(test.iter()).enumerate() {
        if a != b {
            diffs += 1;
        }
    }

    if diffs == 0 {
        println!("\nFiles are IDENTICAL!");
    } else {
        println!("\n{} bytes differ", diffs);

        let key = FSB_KEY;
        let mut dec_orig = orig.clone();
        let mut dec_test = test.clone();
        for (i, byte) in dec_orig.iter_mut().enumerate() {
            *byte = fsbdec(*byte) ^ key[i % key.len()];
        }
        for (i, byte) in dec_test.iter_mut().enumerate() {
            *byte = fsbdec(*byte) ^ key[i % key.len()];
        }

        println!("\nFirst 10 differences (decrypted):");
        let mut shown = 0;
        for (i, (a, b)) in dec_orig.iter().zip(dec_test.iter()).enumerate() {
            if a != b && shown < 10 {
                println!("  Byte {}: orig={:02X} test={:02X}", i, a, b);
                shown += 1;
            }
        }
    }
}
