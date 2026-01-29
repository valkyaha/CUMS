use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::Command;
use crate::crypto::{self, FSB_KEY};

const FSB4_MAGIC: &[u8; 4] = b"FSB4";
const FSB5_MAGIC: &[u8; 4] = b"FSB5";
const FSB5_HEADER_SIZE: usize = 60;
const FREQUENCY_TABLE: [u32; 16] = [
    4000, 8000, 11000, 11025, 16000, 22050, 24000, 32000,
    44100, 48000, 96000, 192000, 0, 0, 0, 0,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version { Fsb4, Fsb5 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Codec {
    None = 0, Pcm8 = 1, Pcm16 = 2, Pcm24 = 3, Pcm32 = 4, PcmFloat = 5,
    GcAdpcm = 6, ImaAdpcm = 7, Vag = 8, Hevag = 9, Xma = 10, Mpeg = 11,
    Celt = 12, At9 = 13, Xwma = 14, Vorbis = 15,
}

impl Codec {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(Self::None), 1 => Some(Self::Pcm8), 2 => Some(Self::Pcm16),
            3 => Some(Self::Pcm24), 4 => Some(Self::Pcm32), 5 => Some(Self::PcmFloat),
            6 => Some(Self::GcAdpcm), 7 => Some(Self::ImaAdpcm), 8 => Some(Self::Vag),
            9 => Some(Self::Hevag), 10 => Some(Self::Xma), 11 => Some(Self::Mpeg),
            12 => Some(Self::Celt), 13 => Some(Self::At9), 14 => Some(Self::Xwma),
            15 => Some(Self::Vorbis), _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Mpeg => "mp3",
            Self::Vorbis => "ogg",
            Self::Pcm8 | Self::Pcm16 | Self::Pcm24 | Self::Pcm32 | Self::PcmFloat => "wav",
            _ => "bin",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encryption { None, Aes, Fsbext }

#[derive(Debug, Clone, Copy)]
pub struct Fsb4Mode(pub u32);

impl Fsb4Mode {
    pub fn is_stereo(&self) -> bool { self.0 & 0x00400000 != 0 }
    pub fn has_loop_points(&self) -> bool { self.0 & 0x00000008 != 0 }
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub index: usize,
    pub name: Option<String>,
    pub frequency: u32,
    pub channels: u32,
    pub samples: u64,
    pub data_offset: u64,
    pub data_size: u64,
    pub loop_start: Option<u32>,
    pub loop_end: Option<u32>,
    pub vorbis_crc: Option<u32>,
    pub vorbis_seek_table: Option<Vec<u32>>,
    pub mode: Option<Fsb4Mode>,
}

impl Sample {
    pub fn duration(&self) -> f64 {
        if self.frequency > 0 { self.samples as f64 / self.frequency as f64 } else { 0.0 }
    }
}

#[derive(Debug)]
pub struct FsbBank {
    pub version: Version,
    pub codec: Codec,
    pub samples: Vec<Sample>,
    pub encryption: Encryption,
    pub data: Vec<u8>,
    pub header_size: usize,
    pub sample_headers_size: u32,
    pub name_table_size: u32,
    pub data_size: u32,
    pub flags: u32,
    pub fsb5_mode: u32,
}

impl FsbBank {
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Self::from_bytes(data)
    }

    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        if data.len() < 8 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "File too small"));
        }

        let version = Self::detect_version(&data)?;
        match version {
            Version::Fsb4 => Self::parse_fsb4(data),
            Version::Fsb5 => Self::parse_fsb5(data),
        }
    }

    fn detect_version(data: &[u8]) -> io::Result<Version> {
        if &data[0..4] == FSB4_MAGIC {
            return Ok(Version::Fsb4);
        }
        if &data[0..4] == FSB5_MAGIC {
            return Ok(Version::Fsb5);
        }

        let mut test = data[0..32].to_vec();
        crypto::decrypt_aes_block(&mut test, FSB_KEY);
        if &test[0..4] == FSB5_MAGIC {
            return Ok(Version::Fsb5);
        }

        let mut test2 = data[0..32].to_vec();
        crypto::fsbext_decrypt(&mut test2, FSB_KEY);
        if &test2[0..4] == FSB5_MAGIC {
            return Ok(Version::Fsb5);
        }

        Err(io::Error::new(io::ErrorKind::InvalidData, "Unknown format"))
    }

    fn parse_fsb4(data: Vec<u8>) -> io::Result<Self> {
        let mut cursor = Cursor::new(&data);
        cursor.seek(SeekFrom::Start(4))?;

        let sample_count = cursor.read_u32::<LittleEndian>()?;
        let sample_headers_size = cursor.read_u32::<LittleEndian>()?;
        let data_size = cursor.read_u32::<LittleEndian>()?;
        let _version = cursor.read_u32::<LittleEndian>()?;
        let flags = cursor.read_u32::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(24))?;

        let header_size = 48usize;
        let data_offset = header_size + sample_headers_size as usize;
        let mut samples = Vec::with_capacity(sample_count as usize);
        let mut current_data_offset = data_offset as u64;

        for i in 0..sample_count as usize {
            let _entry_size = cursor.read_u16::<LittleEndian>()?;
            let mut name_bytes = [0u8; 30];
            cursor.read_exact(&mut name_bytes)?;
            let name = String::from_utf8_lossy(&name_bytes).trim_end_matches('\0').to_string();

            let sample_count_field = cursor.read_u32::<LittleEndian>()?;
            let compressed_size = cursor.read_u32::<LittleEndian>()?;
            let loop_start = cursor.read_u32::<LittleEndian>()?;
            let loop_end = cursor.read_u32::<LittleEndian>()?;
            let mode = Fsb4Mode(cursor.read_u32::<LittleEndian>()?);
            let def_freq = cursor.read_u32::<LittleEndian>()?;
            cursor.seek(SeekFrom::Current(24))?;

            samples.push(Sample {
                index: i,
                name: Some(name),
                frequency: if def_freq > 0 { def_freq } else { 44100 },
                channels: if mode.is_stereo() { 2 } else { 1 },
                samples: sample_count_field as u64,
                data_offset: current_data_offset,
                data_size: compressed_size as u64,
                loop_start: if mode.has_loop_points() { Some(loop_start) } else { None },
                loop_end: if mode.has_loop_points() { Some(loop_end) } else { None },
                vorbis_crc: None,
                vorbis_seek_table: None,
                mode: Some(mode),
            });
            current_data_offset += compressed_size as u64;
        }

        let codec = if flags & 0x00200000 != 0 { Codec::Mpeg } else { Codec::Pcm16 };

        Ok(FsbBank {
            version: Version::Fsb4,
            codec,
            samples,
            encryption: Encryption::None,
            data,
            header_size,
            sample_headers_size,
            name_table_size: 0,
            data_size,
            flags,
            fsb5_mode: 0,
        })
    }

    fn parse_fsb5(mut data: Vec<u8>) -> io::Result<Self> {
        let encryption = if &data[0..4] == FSB5_MAGIC {
            Encryption::None
        } else {
            let mut test = data[0..32].to_vec();
            crypto::decrypt_aes_block(&mut test, FSB_KEY);
            if &test[0..4] == FSB5_MAGIC {
                Encryption::Aes
            } else {
                Encryption::Fsbext
            }
        };

        match encryption {
            Encryption::None => {}
            Encryption::Aes => crypto::decrypt_aes_block(&mut data[0..32], FSB_KEY),
            Encryption::Fsbext => crypto::fsbext_decrypt(&mut data, FSB_KEY),
        }

        let (sample_count, sample_headers_size, name_table_size, data_size, codec_raw, fsb5_mode, flags) = {
            let mut cursor = Cursor::new(&data);
            cursor.seek(SeekFrom::Start(4))?;
            let _version = cursor.read_u32::<LittleEndian>()?;
            (
                cursor.read_u32::<LittleEndian>()?,
                cursor.read_u32::<LittleEndian>()?,
                cursor.read_u32::<LittleEndian>()?,
                cursor.read_u32::<LittleEndian>()?,
                cursor.read_u32::<LittleEndian>()?,
                cursor.read_u32::<LittleEndian>()?,
                cursor.read_u32::<LittleEndian>()?,
            )
        };

        let codec = Codec::from_u32(codec_raw)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Unknown codec"))?;

        let data_offset = FSB5_HEADER_SIZE as u64 + sample_headers_size as u64 + name_table_size as u64;

        if encryption == Encryption::Aes {
            let start = data_offset as usize;
            let end = (start + data_size as usize).min(data.len());
            crypto::decrypt_aes_data(&mut data[start..end], FSB_KEY);
        }

        let mut cursor = Cursor::new(&data);
        cursor.seek(SeekFrom::Start(FSB5_HEADER_SIZE as u64))?;
        let mut samples = Vec::with_capacity(sample_count as usize);

        for i in 0..sample_count as usize {
            let mode = cursor.read_u64::<LittleEndian>()?;
            let has_chunks = (mode & 1) != 0;
            let freq_index = ((mode >> 1) & 0xF) as usize;
            let channels = if (mode >> 5) & 1 != 0 { 2 } else { 1 };
            let sample_data_offset = ((mode >> 6) & 0x0FFFFFFF) * 16;
            let sample_count_val = (mode >> 34) & 0x3FFFFFFF;

            let frequency = FREQUENCY_TABLE.get(freq_index).copied().unwrap_or(44100);
            let mut vorbis_crc = None;
            let mut vorbis_seek_table = None;
            let mut loop_start = None;
            let mut loop_end = None;

            if has_chunks {
                loop {
                    let chunk_header = cursor.read_u32::<LittleEndian>()?;
                    let more_chunks = (chunk_header & 1) != 0;
                    let chunk_size = ((chunk_header >> 1) & 0xFFFFFF) as usize;
                    let chunk_type = (chunk_header >> 25) & 0x7F;
                    let chunk_start = cursor.position();

                    match chunk_type {
                        3 => {
                            loop_start = Some(cursor.read_u32::<LittleEndian>()?);
                            loop_end = Some(cursor.read_u32::<LittleEndian>()?);
                        }
                        11 => {
                            vorbis_crc = Some(cursor.read_u32::<LittleEndian>()?);
                            let seek_count = (chunk_size - 4) / 4;
                            let mut table = Vec::with_capacity(seek_count);
                            for _ in 0..seek_count {
                                table.push(cursor.read_u32::<LittleEndian>()?);
                            }
                            vorbis_seek_table = Some(table);
                        }
                        _ => {}
                    }
                    cursor.seek(SeekFrom::Start(chunk_start + chunk_size as u64))?;
                    if !more_chunks { break; }
                }
            }

            samples.push(Sample {
                index: i,
                name: None,
                frequency,
                channels,
                samples: sample_count_val,
                data_offset: data_offset + sample_data_offset,
                data_size: 0,
                loop_start,
                loop_end,
                vorbis_crc,
                vorbis_seek_table,
                mode: None,
            });
        }

        for i in 0..samples.len() {
            let next_offset = if i + 1 < samples.len() {
                samples[i + 1].data_offset
            } else {
                data_offset + data_size as u64
            };
            samples[i].data_size = next_offset.saturating_sub(samples[i].data_offset);
        }

        if name_table_size > 0 {
            let name_table_offset = FSB5_HEADER_SIZE as u64 + sample_headers_size as u64;
            cursor.seek(SeekFrom::Start(name_table_offset))?;
            let mut offsets = Vec::with_capacity(samples.len());
            for _ in 0..samples.len() {
                offsets.push(cursor.read_u32::<LittleEndian>()?);
            }
            for (i, &offset) in offsets.iter().enumerate() {
                cursor.seek(SeekFrom::Start(name_table_offset + offset as u64))?;
                let mut name_bytes = Vec::new();
                loop {
                    let b = cursor.read_u8()?;
                    if b == 0 { break; }
                    name_bytes.push(b);
                }
                if let Ok(name) = String::from_utf8(name_bytes) {
                    samples[i].name = Some(name);
                }
            }
        }

        Ok(FsbBank {
            version: Version::Fsb5,
            codec,
            samples,
            encryption,
            data,
            header_size: FSB5_HEADER_SIZE,
            sample_headers_size,
            name_table_size,
            data_size,
            flags,
            fsb5_mode,
        })
    }

    pub fn sample_data(&self, index: usize) -> io::Result<&[u8]> {
        let sample = self.samples.get(index)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Sample not found"))?;
        let start = sample.data_offset as usize;
        let end = start + sample.data_size as usize;
        if end > self.data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Sample data out of bounds"));
        }
        Ok(&self.data[start..end])
    }

    pub fn save<P: AsRef<Path>>(&self, path: P, encrypt: bool) -> io::Result<()> {
        match self.version {
            Version::Fsb4 => self.save_fsb4(path),
            Version::Fsb5 => self.save_fsb5(path, encrypt),
        }
    }

    fn save_fsb4<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut output = Vec::new();
        output.extend_from_slice(FSB4_MAGIC);
        output.write_u32::<LittleEndian>(self.samples.len() as u32)?;

        let sample_header_size = 80u16;
        let new_sample_headers_size = self.samples.len() as u32 * sample_header_size as u32;
        output.write_u32::<LittleEndian>(new_sample_headers_size)?;

        let new_data_size: u64 = self.samples.iter().map(|s| s.data_size).sum();
        output.write_u32::<LittleEndian>(new_data_size as u32)?;
        output.write_u32::<LittleEndian>(0x00040001)?;
        output.write_u32::<LittleEndian>(self.flags)?;

        if self.data.len() >= 48 {
            output.extend_from_slice(&self.data[24..48]);
        } else {
            output.extend_from_slice(&[0u8; 24]);
        }

        for sample in &self.samples {
            output.write_u16::<LittleEndian>(sample_header_size)?;
            let name_bytes = sample.name.as_deref().unwrap_or("").as_bytes();
            let mut name_buf = [0u8; 30];
            let copy_len = name_bytes.len().min(29);
            name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
            output.extend_from_slice(&name_buf);

            output.write_u32::<LittleEndian>(sample.samples as u32)?;
            output.write_u32::<LittleEndian>(sample.data_size as u32)?;
            output.write_u32::<LittleEndian>(sample.loop_start.unwrap_or(0))?;
            output.write_u32::<LittleEndian>(sample.loop_end.unwrap_or(sample.samples as u32))?;

            let mode = sample.mode.map(|m| m.0).unwrap_or_else(|| {
                if sample.channels == 2 { 0x00400000 } else { 0x00020000 }
            });
            output.write_u32::<LittleEndian>(mode)?;
            output.write_u32::<LittleEndian>(sample.frequency)?;
            output.write_u16::<LittleEndian>(255)?;
            output.write_u16::<LittleEndian>(128)?;
            output.write_u16::<LittleEndian>(128)?;
            output.write_u16::<LittleEndian>(sample.channels as u16)?;
            output.write_f32::<LittleEndian>(1.0)?;
            output.write_f32::<LittleEndian>(10000.0)?;
            output.write_u32::<LittleEndian>(0)?;
            output.write_u16::<LittleEndian>(0)?;
            output.write_u16::<LittleEndian>(0)?;
        }

        for sample in &self.samples {
            let start = sample.data_offset as usize;
            let end = start + sample.data_size as usize;
            if end <= self.data.len() {
                output.extend_from_slice(&self.data[start..end]);
            }
        }

        let mut file = File::create(path)?;
        file.write_all(&output)
    }

    fn save_fsb5<P: AsRef<Path>>(&self, path: P, encrypt: bool) -> io::Result<()> {
        let mut output = Vec::new();
        let mut audio_data = Vec::new();
        let mut sample_data_offsets = Vec::new();

        for sample in &self.samples {
            while audio_data.len() % 32 != 0 { audio_data.push(0); }
            sample_data_offsets.push(audio_data.len() as u64);
            let start = sample.data_offset as usize;
            let end = start + sample.data_size as usize;
            if end <= self.data.len() {
                audio_data.extend_from_slice(&self.data[start..end]);
            }
        }
        while audio_data.len() % 32 != 0 { audio_data.push(0); }

        let mut sample_headers = Vec::new();
        for (i, sample) in self.samples.iter().enumerate() {
            let data_offset = sample_data_offsets[i] / 16;
            let has_chunks = sample.vorbis_crc.is_some() || sample.loop_start.is_some();
            let freq_index = frequency_to_index(sample.frequency);
            let channels_bit = if sample.channels > 1 { 1u64 } else { 0u64 };

            let mut mode: u64 = 0;
            if has_chunks { mode |= 1; }
            mode |= (freq_index as u64 & 0xF) << 1;
            mode |= channels_bit << 5;
            mode |= (data_offset & 0x0FFFFFFF) << 6;
            mode |= (sample.samples & 0x3FFFFFFF) << 34;
            sample_headers.extend_from_slice(&mode.to_le_bytes());

            if has_chunks {
                let has_vorbis = sample.vorbis_crc.is_some();

                if let (Some(start), Some(end)) = (sample.loop_start, sample.loop_end) {
                    let chunk_header: u32 = (if has_vorbis { 1 } else { 0 }) | (8u32 << 1) | (3u32 << 25);
                    sample_headers.extend_from_slice(&chunk_header.to_le_bytes());
                    sample_headers.extend_from_slice(&start.to_le_bytes());
                    sample_headers.extend_from_slice(&end.to_le_bytes());
                }

                if let Some(crc) = sample.vorbis_crc {
                    let seek_table = sample.vorbis_seek_table.as_ref();
                    let chunk_data_size = 4 + seek_table.map(|t| t.len() * 4).unwrap_or(0);
                    let chunk_header: u32 = ((chunk_data_size as u32 & 0xFFFFFF) << 1) | (11u32 << 25);
                    sample_headers.extend_from_slice(&chunk_header.to_le_bytes());
                    sample_headers.extend_from_slice(&crc.to_le_bytes());
                    if let Some(table) = seek_table {
                        for entry in table {
                            sample_headers.extend_from_slice(&entry.to_le_bytes());
                        }
                    }
                }
            }
        }

        let name_table_start = self.header_size + self.sample_headers_size as usize;
        let name_table_end = name_table_start + self.name_table_size as usize;
        let name_table = if name_table_end <= self.data.len() {
            self.data[name_table_start..name_table_end].to_vec()
        } else {
            Vec::new()
        };

        let new_sample_headers_size = sample_headers.len() as u32;
        let new_data_size = audio_data.len() as u32;

        output.extend_from_slice(FSB5_MAGIC);
        output.write_u32::<LittleEndian>(1)?;
        output.write_u32::<LittleEndian>(self.samples.len() as u32)?;
        output.write_u32::<LittleEndian>(new_sample_headers_size)?;
        output.write_u32::<LittleEndian>(self.name_table_size)?;
        output.write_u32::<LittleEndian>(new_data_size)?;
        output.write_u32::<LittleEndian>(self.codec as u32)?;
        output.write_u32::<LittleEndian>(self.fsb5_mode)?;
        output.write_u32::<LittleEndian>(self.flags)?;

        if self.data.len() >= 60 {
            output.extend_from_slice(&self.data[36..60]);
        } else {
            output.extend_from_slice(&[0u8; 24]);
        }

        output.extend_from_slice(&sample_headers);
        output.extend_from_slice(&name_table);
        output.extend_from_slice(&audio_data);

        if encrypt {
            match self.encryption {
                Encryption::None | Encryption::Aes => {
                    crypto::encrypt_aes_block(&mut output[0..32], FSB_KEY);
                    let data_offset = FSB5_HEADER_SIZE + new_sample_headers_size as usize + self.name_table_size as usize;
                    let data_end = data_offset + new_data_size as usize;
                    if data_end <= output.len() {
                        crypto::encrypt_aes_data(&mut output[data_offset..data_end], FSB_KEY);
                    }
                }
                Encryption::Fsbext => {
                    crypto::fsbext_encrypt(&mut output, FSB_KEY);
                }
            }
        }

        let mut file = File::create(path)?;
        file.write_all(&output)
    }

    pub fn extract_mp3(&self, index: usize) -> io::Result<Vec<u8>> {
        if self.codec != Codec::Mpeg {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Not MPEG codec"));
        }
        let sample = self.samples.get(index)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Sample not found"))?;
        crate::audio::mp3::extract_mp3_from_fsb4(
            self.sample_data(index)?,
            sample.frequency,
            sample.channels,
        )
    }

    pub fn extract_audio(&self, index: usize) -> io::Result<(Vec<u8>, &'static str)> {
        match self.codec {
            Codec::Mpeg => Ok((self.extract_mp3(index)?, "mp3")),
            Codec::Vorbis => Ok((self.sample_data(index)?.to_vec(), "vorbis_raw")),
            Codec::Pcm16 => {
                let sample = &self.samples[index];
                let raw = self.sample_data(index)?;
                Ok((create_wav_header(raw, sample.frequency, sample.channels as u16, 16), "wav"))
            }
            _ => Ok((self.sample_data(index)?.to_vec(), "bin"))
        }
    }

    pub fn replace_sample<P: AsRef<Path>>(&mut self, index: usize, audio_path: P, temp_dir: P) -> io::Result<()> {
        match self.version {
            Version::Fsb4 => self.replace_sample_fsb4(index, audio_path, temp_dir),
            Version::Fsb5 => Err(io::Error::new(io::ErrorKind::InvalidData, "Use replace_sample_fsb5 for FSB5")),
        }
    }

    fn replace_sample_fsb4<P: AsRef<Path>>(&mut self, index: usize, audio_path: P, temp_dir: P) -> io::Result<()> {
        if index >= self.samples.len() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Sample index out of bounds"));
        }

        let temp_dir = temp_dir.as_ref();
        std::fs::create_dir_all(temp_dir)?;
        let new_mp3_data = prepare_mp3_data(audio_path.as_ref(), temp_dir)?;
        let mp3_info = crate::audio::mp3::get_mp3_info(&new_mp3_data);

        let old_size = self.samples[index].data_size as usize;
        let old_offset = self.samples[index].data_offset as usize;
        let new_size = new_mp3_data.len();
        let size_diff = new_size as i64 - old_size as i64;

        let mut new_data = Vec::new();
        new_data.extend_from_slice(&self.data[..old_offset]);
        new_data.extend_from_slice(&new_mp3_data);
        new_data.extend_from_slice(&self.data[old_offset + old_size..]);

        self.samples[index].data_size = new_size as u64;
        if let Some((sample_rate, channels, _)) = mp3_info {
            self.samples[index].frequency = sample_rate;
            self.samples[index].channels = channels;
        }

        for i in (index + 1)..self.samples.len() {
            self.samples[i].data_offset = (self.samples[i].data_offset as i64 + size_diff) as u64;
        }

        self.data_size = (self.data_size as i64 + size_diff) as u32;
        self.data = new_data;
        Ok(())
    }
}

fn frequency_to_index(freq: u32) -> usize {
    match freq {
        4000 => 0, 8000 => 1, 11000 => 2, 11025 => 3, 16000 => 4, 22050 => 5,
        24000 => 6, 32000 => 7, 44100 => 8, 48000 => 9, 96000 => 10, 192000 => 11,
        _ => 8,
    }
}

fn create_wav_header(pcm_data: &[u8], sample_rate: u32, channels: u16, bits_per_sample: u16) -> Vec<u8> {
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_size = pcm_data.len() as u32;
    let file_size = 36 + data_size;

    let mut wav = Vec::with_capacity(44 + pcm_data.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(pcm_data);
    wav
}

fn prepare_mp3_data<P: AsRef<Path>>(audio_path: P, temp_dir: P) -> io::Result<Vec<u8>> {
    let audio_path = audio_path.as_ref();
    let ext = audio_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    if ext == "mp3" {
        return std::fs::read(audio_path);
    }

    let temp_mp3 = temp_dir.as_ref().join("converted.mp3");
    let ffmpeg = find_ffmpeg().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "FFmpeg not found"))?;

    let output = Command::new(&ffmpeg)
        .args(["-y", "-i", &audio_path.to_string_lossy(), "-acodec", "libmp3lame", "-ab", "192k", "-ar", "44100", &temp_mp3.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "FFmpeg conversion failed"));
    }

    let data = std::fs::read(&temp_mp3)?;
    let _ = std::fs::remove_file(&temp_mp3);
    Ok(data)
}

fn find_ffmpeg() -> Option<std::path::PathBuf> {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let local = exe_dir.join("ffmpeg.exe");
            if local.exists() { return Some(local); }
        }
    }
    if Command::new("ffmpeg").arg("-version").output().is_ok() {
        return Some(std::path::PathBuf::from("ffmpeg"));
    }
    None
}
