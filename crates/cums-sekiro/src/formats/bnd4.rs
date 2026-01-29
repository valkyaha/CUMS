use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

const BND4_MAGIC: &[u8; 4] = b"BND4";

#[derive(Debug, Clone)]
pub struct Bnd4Entry {
    pub flags: u8,
    pub id: i32,
    pub name: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct Bnd4 {
    pub version: String,
    pub flags: u8,
    pub big_endian: bool,
    pub bit_big_endian: bool,
    pub unicode: bool,
    pub extended: u8,
    pub entries: Vec<Bnd4Entry>,
}

impl Bnd4 {
    pub fn read(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let mut magic = [0u8; 4];
        cursor.read_exact(&mut magic)?;
        if &magic != BND4_MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid BND4 magic"));
        }

        let flag1 = cursor.read_u8()?;
        let flag2 = cursor.read_u8()?;
        let _unk06 = cursor.read_u8()?;
        let _unk07 = cursor.read_u8()?;

        let big_endian = flag1 & 0x01 != 0;
        let bit_big_endian = flag1 & 0x01 == 0 && flag1 & 0x80 != 0;

        macro_rules! read_u32 {
            ($cursor:expr, $be:expr) => {
                if $be { $cursor.read_u32::<BigEndian>()? } else { $cursor.read_u32::<LittleEndian>()? }
            };
        }
        macro_rules! read_i32 {
            ($cursor:expr, $be:expr) => {
                if $be { $cursor.read_i32::<BigEndian>()? } else { $cursor.read_i32::<LittleEndian>()? }
            };
        }
        macro_rules! read_u64 {
            ($cursor:expr, $be:expr) => {
                if $be { $cursor.read_u64::<BigEndian>()? } else { $cursor.read_u64::<LittleEndian>()? }
            };
        }
        macro_rules! read_i64 {
            ($cursor:expr, $be:expr) => {
                if $be { $cursor.read_i64::<BigEndian>()? } else { $cursor.read_i64::<LittleEndian>()? }
            };
        }

        let entry_count = read_i32!(cursor, big_endian);
        let _header_size = read_u64!(cursor, big_endian);

        let mut version_bytes = [0u8; 8];
        cursor.read_exact(&mut version_bytes)?;
        let version = String::from_utf8_lossy(&version_bytes).trim_end_matches('\0').to_string();

        let _entry_header_size = read_u64!(cursor, big_endian);
        let _data_offset = read_u64!(cursor, big_endian);

        let unicode = read_u32!(cursor, big_endian) == 1;
        let extended = cursor.read_u8()?;
        let _unk35 = cursor.read_u8()?;
        let _unk36 = cursor.read_u8()?;
        let _unk37 = cursor.read_u8()?;

        if extended == 0x10 {
            let _hash_groups_offset = read_u64!(cursor, big_endian);
        }

        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let entry_flags = cursor.read_u8()?;
            let _unk01 = cursor.read_u8()?;
            let _unk02 = cursor.read_u8()?;
            let _unk03 = cursor.read_u8()?;

            let _minus_one = read_i32!(cursor, big_endian);
            let compressed_size = read_i64!(cursor, big_endian);
            let uncompressed_size = if extended == 0x10 {
                read_u64!(cursor, big_endian)
            } else {
                compressed_size as u64
            };
            let data_offset = read_u64!(cursor, big_endian);
            let file_id = read_i32!(cursor, big_endian);
            let name_offset = read_u32!(cursor, big_endian);

            if extended == 0x10 {
                let _unk24 = read_u64!(cursor, big_endian);
            }

            let pos = cursor.position();
            cursor.seek(SeekFrom::Start(name_offset as u64))?;

            let name = if unicode {
                read_wide_string(&mut cursor)?
            } else {
                read_string(&mut cursor)?
            };

            cursor.seek(SeekFrom::Start(data_offset))?;
            let data_len = if compressed_size > 0 {
                compressed_size as usize
            } else {
                uncompressed_size as usize
            };
            let mut file_data = vec![0u8; data_len];
            cursor.read_exact(&mut file_data)?;

            cursor.seek(SeekFrom::Start(pos))?;

            entries.push(Bnd4Entry {
                flags: entry_flags,
                id: file_id,
                name,
                uncompressed_size,
                compressed_size: compressed_size as u64,
                data: file_data,
            });
        }

        Ok(Bnd4 { version, flags: flag2, big_endian, bit_big_endian, unicode, extended, entries })
    }

    pub fn write(&self) -> io::Result<Vec<u8>> {
        let mut output = Vec::new();
        let mut cursor = Cursor::new(&mut output);

        cursor.write_all(BND4_MAGIC)?;

        let flag1 = if self.big_endian { 0x01 } else { 0x00 } | if self.bit_big_endian { 0x80 } else { 0x00 };
        cursor.write_u8(flag1)?;
        cursor.write_u8(self.flags)?;
        cursor.write_u8(0)?;
        cursor.write_u8(0)?;

        macro_rules! write_u32 {
            ($cursor:expr, $val:expr, $be:expr) => {
                if $be { $cursor.write_u32::<BigEndian>($val)? } else { $cursor.write_u32::<LittleEndian>($val)? }
            };
        }
        macro_rules! write_i32 {
            ($cursor:expr, $val:expr, $be:expr) => {
                if $be { $cursor.write_i32::<BigEndian>($val)? } else { $cursor.write_i32::<LittleEndian>($val)? }
            };
        }
        macro_rules! write_u64 {
            ($cursor:expr, $val:expr, $be:expr) => {
                if $be { $cursor.write_u64::<BigEndian>($val)? } else { $cursor.write_u64::<LittleEndian>($val)? }
            };
        }
        macro_rules! write_i64 {
            ($cursor:expr, $val:expr, $be:expr) => {
                if $be { $cursor.write_i64::<BigEndian>($val)? } else { $cursor.write_i64::<LittleEndian>($val)? }
            };
        }

        let be = self.big_endian;

        write_i32!(cursor, self.entries.len() as i32, be);

        let header_size_pos = cursor.position();
        write_u64!(cursor, 0, be);

        let mut version_bytes = [0u8; 8];
        let version_src = self.version.as_bytes();
        version_bytes[..version_src.len().min(8)].copy_from_slice(&version_src[..version_src.len().min(8)]);
        cursor.write_all(&version_bytes)?;

        let entry_header_size = if self.extended == 0x10 { 36u64 } else { 24u64 };
        write_u64!(cursor, entry_header_size, be);

        let data_offset_pos = cursor.position();
        write_u64!(cursor, 0, be);

        write_u32!(cursor, if self.unicode { 1 } else { 0 }, be);

        cursor.write_u8(self.extended)?;
        cursor.write_all(&[0u8; 3])?;

        if self.extended == 0x10 {
            write_u64!(cursor, 0, be);
        }

        let header_end = cursor.position();

        let entry_headers_size = self.entries.len() as u64 * entry_header_size;
        let names_offset = header_end + entry_headers_size;

        let mut current_name_offset = names_offset;
        let mut name_offsets = Vec::new();
        for entry in &self.entries {
            name_offsets.push(current_name_offset);
            current_name_offset += if self.unicode {
                (entry.name.len() + 1) * 2
            } else {
                entry.name.len() + 1
            } as u64;
        }

        let data_start = (current_name_offset + 15) & !15;

        let mut current_data_offset = data_start;
        let mut data_offsets = Vec::new();
        for entry in &self.entries {
            data_offsets.push(current_data_offset);
            current_data_offset += entry.data.len() as u64;
            current_data_offset = (current_data_offset + 15) & !15;
        }

        for (i, entry) in self.entries.iter().enumerate() {
            cursor.write_u8(entry.flags)?;
            cursor.write_all(&[0u8; 3])?;
            write_i32!(cursor, -1, be);
            write_i64!(cursor, entry.compressed_size as i64, be);
            if self.extended == 0x10 {
                write_u64!(cursor, entry.uncompressed_size, be);
            }
            write_u64!(cursor, data_offsets[i], be);
            write_i32!(cursor, entry.id, be);
            write_u32!(cursor, name_offsets[i] as u32, be);
            if self.extended == 0x10 {
                write_u64!(cursor, 0, be);
            }
        }

        for entry in &self.entries {
            if self.unicode {
                write_wide_string(&mut cursor, &entry.name)?;
            } else {
                cursor.write_all(entry.name.as_bytes())?;
                cursor.write_u8(0)?;
            }
        }

        while cursor.position() < data_start {
            cursor.write_u8(0)?;
        }

        for (i, entry) in self.entries.iter().enumerate() {
            cursor.seek(SeekFrom::Start(data_offsets[i]))?;
            cursor.write_all(&entry.data)?;
        }

        let total_size = cursor.position();
        cursor.seek(SeekFrom::Start(header_size_pos))?;
        write_u64!(cursor, header_end, be);
        cursor.seek(SeekFrom::Start(data_offset_pos))?;
        write_u64!(cursor, data_start, be);

        drop(cursor);
        output.resize(total_size as usize, 0);

        Ok(output)
    }

    pub fn get_entry(&self, name: &str) -> Option<&Bnd4Entry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn get_entry_mut(&mut self, name: &str) -> Option<&mut Bnd4Entry> {
        self.entries.iter_mut().find(|e| e.name == name)
    }
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> io::Result<String> {
    let mut bytes = Vec::new();
    loop {
        let b = cursor.read_u8()?;
        if b == 0 { break; }
        bytes.push(b);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_wide_string(cursor: &mut Cursor<&[u8]>) -> io::Result<String> {
    let mut chars = Vec::new();
    loop {
        let c = cursor.read_u16::<LittleEndian>()?;
        if c == 0 { break; }
        chars.push(c);
    }
    Ok(String::from_utf16_lossy(&chars))
}

fn write_wide_string(cursor: &mut Cursor<&mut Vec<u8>>, s: &str) -> io::Result<()> {
    for c in s.encode_utf16() {
        cursor.write_u16::<LittleEndian>(c)?;
    }
    cursor.write_u16::<LittleEndian>(0)?;
    Ok(())
}
