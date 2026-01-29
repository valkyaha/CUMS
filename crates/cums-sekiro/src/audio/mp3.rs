use std::io::{self, Cursor, Read};
use byteorder::{BigEndian, ReadBytesExt};

const BITRATES_V1_L3: [u32; 16] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0];
const BITRATES_V2_L3: [u32; 16] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0];
const SAMPLE_RATES: [[u32; 4]; 4] = [
    [11025, 12000, 8000, 0],
    [0, 0, 0, 0],
    [22050, 24000, 16000, 0],
    [44100, 48000, 32000, 0],
];

#[derive(Debug, Clone)]
pub struct Mp3FrameHeader {
    pub version: u8,
    pub layer: u8,
    pub crc: bool,
    pub bitrate_index: u8,
    pub sample_rate_index: u8,
    pub padding: bool,
    pub channel_mode: u8,
    pub frame_size: usize,
    pub bitrate: u32,
    pub sample_rate: u32,
}

impl Mp3FrameHeader {
    pub fn parse(header: u32) -> Option<Self> {
        if (header >> 21) != 0x7FF { return None; }

        let version = ((header >> 19) & 0x03) as u8;
        let layer = ((header >> 17) & 0x03) as u8;
        let crc = ((header >> 16) & 0x01) == 0;
        let bitrate_index = ((header >> 12) & 0x0F) as u8;
        let sample_rate_index = ((header >> 10) & 0x03) as u8;
        let padding = ((header >> 9) & 0x01) == 1;
        let channel_mode = ((header >> 6) & 0x03) as u8;

        if version == 1 || layer == 0 || bitrate_index == 0 || bitrate_index == 15 || sample_rate_index == 3 {
            return None;
        }

        let bitrate = if version == 3 {
            match layer {
                3 => BITRATES_V1_L3[bitrate_index as usize],
                _ => return None,
            }
        } else {
            match layer {
                3 => BITRATES_V2_L3[bitrate_index as usize],
                _ => return None,
            }
        };

        let sample_rate = SAMPLE_RATES[version as usize][sample_rate_index as usize];
        if sample_rate == 0 { return None; }

        let frame_size = if layer == 3 {
            let coefficient = if version == 3 { 144 } else { 72 };
            (coefficient * bitrate * 1000 / sample_rate + if padding { 1 } else { 0 }) as usize
        } else {
            return None;
        };

        Some(Mp3FrameHeader {
            version, layer, crc, bitrate_index, sample_rate_index,
            padding, channel_mode, frame_size, bitrate, sample_rate,
        })
    }

    pub fn encode(&self) -> u32 {
        let mut header: u32 = 0x7FF << 21;
        header |= (self.version as u32) << 19;
        header |= (self.layer as u32) << 17;
        header |= if self.crc { 0 } else { 1 } << 16;
        header |= (self.bitrate_index as u32) << 12;
        header |= (self.sample_rate_index as u32) << 10;
        header |= if self.padding { 1 } else { 0 } << 9;
        header |= (self.channel_mode as u32) << 6;
        header
    }
}

pub fn extract_mp3_from_fsb4(data: &[u8], _sample_rate: u32, _channels: u32) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut cursor = Cursor::new(data);
    let data_len = data.len();

    while (cursor.position() as usize) < data_len - 4 {
        let pos = cursor.position() as usize;
        let header_bytes = cursor.read_u32::<BigEndian>()?;

        if let Some(frame) = Mp3FrameHeader::parse(header_bytes) {
            if pos + frame.frame_size <= data_len {
                output.extend_from_slice(&header_bytes.to_be_bytes());
                let frame_data_size = frame.frame_size - 4;
                let mut frame_data = vec![0u8; frame_data_size];
                cursor.read_exact(&mut frame_data)?;
                output.extend_from_slice(&frame_data);
            } else {
                break;
            }
        } else {
            cursor.set_position(pos as u64 + 1);
            if let Some(sync_pos) = find_mp3_sync(&data[pos + 1..]) {
                cursor.set_position((pos + 1 + sync_pos) as u64);
            } else {
                break;
            }
        }
    }

    if output.is_empty() {
        return Ok(data.to_vec());
    }

    Ok(output)
}

fn find_mp3_sync(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(4) {
        if data[i] == 0xFF && (data[i + 1] & 0xE0) == 0xE0 {
            let header = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
            if Mp3FrameHeader::parse(header).is_some() {
                return Some(i);
            }
        }
    }
    None
}

pub fn extract_fsb4_mp3_fmod(data: &[u8], channels: u32) -> io::Result<Vec<u8>> {
    if channels == 1 {
        extract_mp3_from_fsb4(data, 44100, 1)
    } else {
        let direct = extract_mp3_from_fsb4(data, 44100, channels)?;
        if !direct.is_empty() && has_valid_mp3_frames(&direct) {
            return Ok(direct);
        }
        Ok(data.to_vec())
    }
}

pub fn has_valid_mp3_frames(data: &[u8]) -> bool {
    if data.len() < 4 { return false; }
    let header = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    Mp3FrameHeader::parse(header).is_some()
}

pub fn get_mp3_info(data: &[u8]) -> Option<(u32, u32, u32)> {
    if data.len() < 4 { return None; }

    for i in 0..data.len().saturating_sub(4) {
        let header = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        if let Some(frame) = Mp3FrameHeader::parse(header) {
            let channels = if frame.channel_mode == 3 { 1 } else { 2 };
            return Some((frame.sample_rate, channels, frame.bitrate));
        }
    }

    None
}

pub fn create_mp3_file(frames: &[u8], _sample_rate: u32, _channels: u32) -> Vec<u8> {
    frames.to_vec()
}
