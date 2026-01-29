use byteorder::{LittleEndian, ReadBytesExt};
use std::fs;
use std::io::{Cursor, Seek, SeekFrom};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let orig_path = r"G:\SteamLibrary\steamapps\common\Sekiro\sound\main.fsb";
    let mod_path = r"G:\SteamLibrary\steamapps\common\Sekiro\mods\sound\main.fsb";

    let orig = fs::read(orig_path)?;
    let modded = fs::read(mod_path)?;

    println!("=== Header Comparison (60 bytes) ===\n");

    // Parse headers
    fn parse_header(data: &[u8]) -> (u32, u32, u32, u32, u32, u32, u32, u32) {
        let mut c = Cursor::new(data);
        c.seek(SeekFrom::Start(4)).unwrap();
        let version = c.read_u32::<LittleEndian>().unwrap();
        let sample_count = c.read_u32::<LittleEndian>().unwrap();
        let sample_headers_size = c.read_u32::<LittleEndian>().unwrap();
        let name_table_size = c.read_u32::<LittleEndian>().unwrap();
        let data_size = c.read_u32::<LittleEndian>().unwrap();
        let codec = c.read_u32::<LittleEndian>().unwrap();
        let zero = c.read_u32::<LittleEndian>().unwrap();
        let flags = c.read_u32::<LittleEndian>().unwrap();
        (
            version,
            sample_count,
            sample_headers_size,
            name_table_size,
            data_size,
            codec,
            zero,
            flags,
        )
    }

    let (ov, osc, osh, ont, ods, oc, oz, of) = parse_header(&orig);
    let (mv, msc, msh, mnt, mds, mc, mz, mf) = parse_header(&modded);

    println!("                    Original    Modified    Diff");
    println!("Version:            {:10}  {:10}", ov, mv);
    println!("Sample count:       {:10}  {:10}", osc, msc);
    println!(
        "Sample headers:     {:10}  {:10}  {:+}",
        osh,
        msh,
        msh as i64 - osh as i64
    );
    println!(
        "Name table size:    {:10}  {:10}  {:+}",
        ont,
        mnt,
        mnt as i64 - ont as i64
    );
    println!(
        "Data size:          {:10}  {:10}  {:+}",
        ods,
        mds,
        mds as i64 - ods as i64
    );
    println!("Codec:              {:10}  {:10}", oc, mc);
    println!("Zero:               {:10}  {:10}", oz, mz);
    println!("Flags:              {:10}  {:10}", of, mf);

    let orig_data_offset = 60 + osh + ont;
    let mod_data_offset = 60 + msh + mnt;
    println!(
        "\nData offset:        {:10}  {:10}  {:+}",
        orig_data_offset,
        mod_data_offset,
        mod_data_offset as i64 - orig_data_offset as i64
    );

    // Check first few sample headers
    println!("\n=== First 3 Sample Headers (hex) ===\n");

    let orig_sh_start = 60usize;
    let mod_sh_start = 60usize;

    for i in 0..3 {
        let orig_start = orig_sh_start + i * 8;
        let mod_start = mod_sh_start + i * 8;

        println!("Sample {} mode bytes:", i);
        println!(
            "  Orig: {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
            orig[orig_start],
            orig[orig_start + 1],
            orig[orig_start + 2],
            orig[orig_start + 3],
            orig[orig_start + 4],
            orig[orig_start + 5],
            orig[orig_start + 6],
            orig[orig_start + 7]
        );
        println!(
            "  Mod:  {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
            modded[mod_start],
            modded[mod_start + 1],
            modded[mod_start + 2],
            modded[mod_start + 3],
            modded[mod_start + 4],
            modded[mod_start + 5],
            modded[mod_start + 6],
            modded[mod_start + 7]
        );
    }

    // Check name table
    println!("\n=== Name Table Start ===\n");
    let orig_nt_start = 60 + osh as usize;
    let mod_nt_start = 60 + msh as usize;

    println!(
        "Original name table starts at offset {} (0x{:X})",
        orig_nt_start, orig_nt_start
    );
    println!(
        "Modified name table starts at offset {} (0x{:X})",
        mod_nt_start, mod_nt_start
    );

    println!("\nFirst 32 bytes of name table:");
    print!("  Orig: ");
    for i in 0..32.min(ont as usize) {
        print!("{:02X} ", orig[orig_nt_start + i]);
    }
    println!();
    print!("  Mod:  ");
    for i in 0..32.min(mnt as usize) {
        print!("{:02X} ", modded[mod_nt_start + i]);
    }
    println!();

    // Check data section start
    println!("\n=== Data Section Start ===\n");
    println!(
        "Original data starts at offset {} (0x{:X})",
        orig_data_offset, orig_data_offset
    );
    println!(
        "Modified data starts at offset {} (0x{:X})",
        mod_data_offset, mod_data_offset
    );

    println!("\nFirst 32 bytes of data section:");
    print!("  Orig: ");
    for i in 0..32 {
        print!("{:02X} ", orig[orig_data_offset as usize + i]);
    }
    println!();
    print!("  Mod:  ");
    for i in 0..32 {
        print!("{:02X} ", modded[mod_data_offset as usize + i]);
    }
    println!();

    // Check hash
    println!("\n=== Hash/Unknown (bytes 36-60) ===\n");
    print!("  Orig: ");
    for b in &orig[36..60] {
        print!("{:02X} ", b);
    }
    println!();
    print!("  Mod:  ");
    for b in &modded[36..60] {
        print!("{:02X} ", b);
    }
    println!();

    Ok(())
}
