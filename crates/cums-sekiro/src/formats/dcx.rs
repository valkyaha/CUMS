use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{self, Cursor, Read, Write};

const DCX_MAGIC: &[u8; 4] = b"DCX\0";
const DCS_MAGIC: &[u8; 4] = b"DCS\0";
const DCP_MAGIC: &[u8; 4] = b"DCP\0";
const DCA_MAGIC: &[u8; 4] = b"DCA\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DcxType {
    None,
    Zlib,
    Dflt,
    Edge,
    Kraken,
}

impl DcxType {
    fn from_magic(magic: &[u8; 4]) -> Option<Self> {
        match magic {
            b"DFLT" => Some(DcxType::Dflt),
            b"EDGE" => Some(DcxType::Edge),
            b"KRAK" => Some(DcxType::Kraken),
            _ => None,
        }
    }

    fn to_magic(&self) -> [u8; 4] {
        match self {
            DcxType::None => *b"\0\0\0\0",
            DcxType::Zlib => *b"DFLT",
            DcxType::Dflt => *b"DFLT",
            DcxType::Edge => *b"EDGE",
            DcxType::Kraken => *b"KRAK",
        }
    }
}

#[derive(Debug)]
pub struct Dcx {
    pub compression: DcxType,
    pub data: Vec<u8>,
}

impl Dcx {
    pub fn is_dcx(data: &[u8]) -> bool {
        data.len() >= 4 && &data[0..4] == DCX_MAGIC
    }

    pub fn decompress(data: &[u8]) -> io::Result<Self> {
        if !Self::is_dcx(data) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a DCX file"));
        }

        let mut cursor = Cursor::new(data);

        let mut magic = [0u8; 4];
        cursor.read_exact(&mut magic)?;

        let _unk04 = cursor.read_u32::<BigEndian>()?;
        let _dcs_offset = cursor.read_u32::<BigEndian>()?;
        let _dcp_offset = cursor.read_u32::<BigEndian>()?;

        cursor.read_exact(&mut magic)?;
        if &magic != DCS_MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid DCS magic"));
        }

        let uncompressed_size = cursor.read_u32::<BigEndian>()?;
        let compressed_size = cursor.read_u32::<BigEndian>()?;

        cursor.read_exact(&mut magic)?;
        if &magic != DCP_MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid DCP magic"));
        }

        let mut compression_magic = [0u8; 4];
        cursor.read_exact(&mut compression_magic)?;

        let compression = DcxType::from_magic(&compression_magic)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData,
                format!("Unknown compression type: {:?}", compression_magic)))?;

        let _dcp_unk08 = cursor.read_u32::<BigEndian>()?;
        let _dcp_unk0c = cursor.read_u32::<BigEndian>()?;
        let _dcp_unk10 = cursor.read_u32::<BigEndian>()?;
        let _dcp_unk14 = cursor.read_u32::<BigEndian>()?;

        if compression == DcxType::Dflt || compression == DcxType::Zlib {
            cursor.read_exact(&mut magic)?;
            if &magic == DCA_MAGIC {
                let _dca_size = cursor.read_u32::<BigEndian>()?;
            }
        }

        let data_offset = cursor.position() as usize;
        let compressed_data = &data[data_offset..data_offset + compressed_size as usize];

        let decompressed = match compression {
            DcxType::Dflt | DcxType::Zlib => {
                let mut decoder = ZlibDecoder::new(compressed_data);
                let mut output = Vec::with_capacity(uncompressed_size as usize);
                match decoder.read_to_end(&mut output) {
                    Ok(_) => output,
                    Err(_) => {
                        use flate2::read::DeflateDecoder;
                        let mut decoder = DeflateDecoder::new(compressed_data);
                        let mut output = Vec::with_capacity(uncompressed_size as usize);
                        decoder.read_to_end(&mut output)?;
                        output
                    }
                }
            }
            DcxType::Kraken => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Kraken/Oodle compression not supported - requires oo2core_6_win64.dll",
                ));
            }
            DcxType::Edge => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Edge compression not yet implemented",
                ));
            }
            DcxType::None => compressed_data.to_vec(),
        };

        Ok(Dcx { compression, data: decompressed })
    }

    pub fn compress(data: &[u8], compression: DcxType) -> io::Result<Vec<u8>> {
        let compressed_data = match compression {
            DcxType::Dflt | DcxType::Zlib => {
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
                encoder.write_all(data)?;
                encoder.finish()?
            }
            DcxType::None => data.to_vec(),
            DcxType::Kraken | DcxType::Edge => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "This compression type is not supported for writing",
                ));
            }
        };

        let mut output = Vec::new();
        let mut cursor = Cursor::new(&mut output);

        cursor.write_all(DCX_MAGIC)?;
        cursor.write_u32::<BigEndian>(0x10000)?;
        cursor.write_u32::<BigEndian>(0x18)?;
        cursor.write_u32::<BigEndian>(0x24)?;

        cursor.write_u32::<BigEndian>(0x24)?;
        cursor.write_u32::<BigEndian>(0x2C)?;

        cursor.write_all(DCS_MAGIC)?;
        cursor.write_u32::<BigEndian>(data.len() as u32)?;
        cursor.write_u32::<BigEndian>(compressed_data.len() as u32)?;

        cursor.write_all(DCP_MAGIC)?;
        cursor.write_all(&compression.to_magic())?;
        cursor.write_u32::<BigEndian>(0x20)?;
        cursor.write_u32::<BigEndian>(0x09)?;
        cursor.write_u32::<BigEndian>(0x00)?;
        cursor.write_u32::<BigEndian>(0x00)?;
        cursor.write_u32::<BigEndian>(0x00)?;
        cursor.write_u32::<BigEndian>(0x00010100)?;

        cursor.write_all(DCA_MAGIC)?;
        cursor.write_u32::<BigEndian>(0x08)?;

        cursor.write_all(&compressed_data)?;

        drop(cursor);
        Ok(output)
    }

    pub fn new(data: Vec<u8>, compression: DcxType) -> Self {
        Dcx { compression, data }
    }
}
