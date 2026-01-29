use aes::cipher::{generic_array::GenericArray, BlockDecrypt, KeyInit};
use aes::Aes128;
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::io::{self, Cursor, Read, Seek, SeekFrom};

const BHD5_MAGIC: &[u8; 4] = b"BHD5";

pub mod keys {
    pub const DS3_KEY: &[u8] = include_bytes!("../keys/ds3.pem");
    pub const FSB_KEY: &[u8] = include_bytes!("../keys/sekiro.pem");
}

#[derive(Debug, Clone)]
pub struct Bhd5Entry {
    pub hash: u32,
    pub size: u32,
    pub offset: u64,
    pub padded_size: u32,
    pub aes_key: Option<Vec<u8>>,
    pub aes_ranges: Vec<(i64, i64)>,
}

#[derive(Debug)]
pub struct Bhd5Bucket {
    pub entries: Vec<Bhd5Entry>,
}

#[derive(Debug)]
pub struct Bhd5 {
    pub version: u32,
    pub salt: Vec<u8>,
    pub buckets: Vec<Bhd5Bucket>,
    pub big_endian: bool,
}

impl Bhd5 {
    pub fn read(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let mut magic = [0u8; 4];
        cursor.read_exact(&mut magic)?;
        if &magic != BHD5_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid BHD5 magic",
            ));
        }

        let endian_check = cursor.read_u32::<LittleEndian>()?;
        let big_endian = endian_check == 0x01000000;

        macro_rules! read_u32 {
            ($cursor:expr) => {
                if big_endian {
                    $cursor.read_u32::<BigEndian>()?
                } else {
                    $cursor.read_u32::<LittleEndian>()?
                }
            };
        }
        macro_rules! read_i32 {
            ($cursor:expr) => {
                if big_endian {
                    $cursor.read_i32::<BigEndian>()?
                } else {
                    $cursor.read_i32::<LittleEndian>()?
                }
            };
        }
        macro_rules! read_u64 {
            ($cursor:expr) => {
                if big_endian {
                    $cursor.read_u64::<BigEndian>()?
                } else {
                    $cursor.read_u64::<LittleEndian>()?
                }
            };
        }
        macro_rules! read_i64 {
            ($cursor:expr) => {
                if big_endian {
                    $cursor.read_i64::<BigEndian>()?
                } else {
                    $cursor.read_i64::<LittleEndian>()?
                }
            };
        }

        cursor.seek(SeekFrom::Start(4))?;
        let _endian_check = read_u32!(cursor);

        let version = read_u32!(cursor);
        let _data_size = read_u32!(cursor);
        let bucket_count = read_u32!(cursor);
        let buckets_offset = read_u32!(cursor);

        let salt_length = read_u32!(cursor);
        let mut salt = vec![0u8; salt_length as usize];
        if salt_length > 0 {
            cursor.read_exact(&mut salt)?;
        }

        cursor.seek(SeekFrom::Start(buckets_offset as u64))?;
        let mut buckets = Vec::with_capacity(bucket_count as usize);

        for _ in 0..bucket_count {
            let entry_count = read_u32!(cursor);
            let entries_offset = read_u32!(cursor);

            let pos = cursor.position();
            cursor.seek(SeekFrom::Start(entries_offset as u64))?;

            let mut entries = Vec::with_capacity(entry_count as usize);
            for _ in 0..entry_count {
                let hash = read_u32!(cursor);
                let size = read_u32!(cursor);
                let offset = read_u64!(cursor);

                let padded_size = if version >= 0x100 {
                    read_u32!(cursor)
                } else {
                    size
                };

                let aes_key_offset = if version >= 0x100 {
                    read_u64!(cursor)
                } else {
                    0
                };

                let mut aes_key = None;
                let mut aes_ranges = Vec::new();

                if aes_key_offset > 0 {
                    let entry_pos = cursor.position();
                    cursor.seek(SeekFrom::Start(aes_key_offset))?;

                    let mut key = vec![0u8; 16];
                    cursor.read_exact(&mut key)?;
                    aes_key = Some(key);

                    let range_count = read_i32!(cursor);
                    for _ in 0..range_count {
                        let start = read_i64!(cursor);
                        let end = read_i64!(cursor);
                        aes_ranges.push((start, end));
                    }

                    cursor.seek(SeekFrom::Start(entry_pos))?;
                }

                entries.push(Bhd5Entry {
                    hash,
                    size,
                    offset,
                    padded_size,
                    aes_key,
                    aes_ranges,
                });
            }

            cursor.seek(SeekFrom::Start(pos))?;
            buckets.push(Bhd5Bucket { entries });
        }

        Ok(Bhd5 {
            version,
            salt,
            buckets,
            big_endian,
        })
    }

    pub fn get_entry(&self, hash: u32) -> Option<&Bhd5Entry> {
        let bucket_index = (hash % self.buckets.len() as u32) as usize;
        self.buckets
            .get(bucket_index)?
            .entries
            .iter()
            .find(|e| e.hash == hash)
    }

    pub fn all_entries(&self) -> Vec<&Bhd5Entry> {
        self.buckets.iter().flat_map(|b| b.entries.iter()).collect()
    }

    pub fn hash_path(path: &str, salt: &[u8]) -> u32 {
        let path_lower = path.to_lowercase().replace('/', "\\");
        let mut hash = 0u32;

        for &b in path_lower.as_bytes() {
            hash = hash.wrapping_mul(37).wrapping_add(b as u32);
        }

        for &b in salt {
            hash = hash.wrapping_mul(37).wrapping_add(b as u32);
        }

        hash
    }
}

pub struct Bdt<'a> {
    data: &'a [u8],
}

impl<'a> Bdt<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Bdt { data }
    }

    pub fn read_entry(&self, entry: &Bhd5Entry) -> io::Result<Vec<u8>> {
        let start = entry.offset as usize;
        let end = start + entry.padded_size as usize;

        if end > self.data.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Entry data out of bounds",
            ));
        }

        let mut data = self.data[start..end].to_vec();

        if let Some(ref key) = entry.aes_key {
            decrypt_aes128_ecb(&mut data, key, &entry.aes_ranges)?;
        }

        data.truncate(entry.size as usize);

        Ok(data)
    }
}

fn decrypt_aes128_ecb(data: &mut [u8], key: &[u8], ranges: &[(i64, i64)]) -> io::Result<()> {
    if key.len() != 16 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid AES key length",
        ));
    }

    let cipher = Aes128::new(GenericArray::from_slice(key));

    if ranges.is_empty() {
        for chunk in data.chunks_exact_mut(16) {
            let block = GenericArray::from_mut_slice(chunk);
            cipher.decrypt_block(block);
        }
    } else {
        for &(start, end) in ranges {
            let start = start as usize;
            let end = (end as usize).min(data.len());
            if start < end {
                for chunk in data[start..end].chunks_exact_mut(16) {
                    let block = GenericArray::from_mut_slice(chunk);
                    cipher.decrypt_block(block);
                }
            }
        }
    }

    Ok(())
}
