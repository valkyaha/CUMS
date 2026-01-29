mod crypto;
mod fsb;
pub mod formats;
pub mod audio;

pub use crypto::FSB_KEY;
pub use fsb::{FsbBank, Sample, Codec, Version, Encryption, Fsb4Mode};

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};
use std::path::Path;
use std::process::Command;
use std::collections::HashMap;
use once_cell::sync::Lazy;

const VORBIS_HEADERS_JSON: &str = include_str!("vorbis_headers.json");

static VORBIS_HEADERS: Lazy<HashMap<u32, Vec<u8>>> = Lazy::new(|| {
    use base64::Engine;
    use serde_json::Value;
    let mut headers = HashMap::new();
    if let Ok(json) = serde_json::from_str::<Value>(VORBIS_HEADERS_JSON) {
        if let Value::Object(map) = json {
            for (crc_str, value) in map {
                if let Ok(crc) = crc_str.parse::<u32>() {
                    if let Some(b64) = value.get("headerBytes").and_then(|v| v.as_str()) {
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                            headers.insert(crc, bytes);
                        }
                    }
                }
            }
        }
    }
    headers
});

#[derive(Debug, Clone)]
pub struct AudioSettings {
    pub volume_db: f32,
    pub pitch_semitones: f32,
    pub speed: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { volume_db: 0.0, pitch_semitones: 0.0, speed: 1.0 }
    }
}

impl AudioSettings {
    pub fn needs_processing(&self) -> bool {
        self.volume_db.abs() > 0.01 || self.pitch_semitones.abs() > 0.01 || (self.speed - 1.0).abs() > 0.01
    }

    pub fn to_ffmpeg_filter(&self) -> Option<String> {
        if !self.needs_processing() { return None; }
        let mut filters = Vec::new();
        if self.volume_db.abs() > 0.01 {
            filters.push(format!("volume={}dB", self.volume_db));
        }
        if self.pitch_semitones.abs() > 0.01 {
            let ratio = 2.0_f32.powf(self.pitch_semitones / 12.0);
            filters.push(format!("asetrate=48000*{:.4},aresample=48000", ratio));
        }
        if (self.speed - 1.0).abs() > 0.01 {
            let mut speed = self.speed.clamp(0.25, 4.0);
            while speed < 0.5 || speed > 2.0 {
                if speed < 0.5 { filters.push("atempo=0.5".into()); speed /= 0.5; }
                else { filters.push("atempo=2.0".into()); speed /= 2.0; }
            }
            filters.push(format!("atempo={:.4}", speed));
        }
        Some(filters.join(","))
    }
}

pub fn get_vorbis_setup_header(crc: u32) -> Option<Vec<u8>> {
    VORBIS_HEADERS.get(&crc).cloned()
}

pub fn rebuild_ogg(bank: &FsbBank, sample: &Sample) -> Result<Vec<u8>, std::io::Error> {
    if bank.codec != Codec::Vorbis {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Not Vorbis"));
    }
    let crc = sample.vorbis_crc.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Missing CRC"))?;
    let setup = get_vorbis_setup_header(crc)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Unknown CRC 0x{:08X}", crc)))?;
    let raw = bank.sample_data(sample.index)?;

    let id_header = generate_vorbis_id_header(sample.frequency, sample.channels as u8);
    let comment_header = generate_vorbis_comment_header();
    build_ogg_file(&id_header, &comment_header, &setup, raw)
}

pub fn extract_mp3(bank: &FsbBank, sample: &Sample) -> Result<Vec<u8>, std::io::Error> {
    bank.extract_mp3(sample.index)
}

pub fn replace_sample(
    bank: &mut FsbBank,
    sample_index: usize,
    audio_path: &Path,
    fsbankcl_path: &Path,
    temp_dir: &Path,
    settings: &AudioSettings,
) -> Result<(), std::io::Error> {
    if bank.version != Version::Fsb5 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Use FsbBank::replace_sample for FSB4"));
    }
    if sample_index >= bank.samples.len() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Sample index out of bounds"));
    }

    let target_freq = bank.samples[sample_index].frequency;
    let target_channels = bank.samples[sample_index].channels;

    let fsbankcl_dir = fsbankcl_path.parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid fsbankcl path"))?;

    let temp_fsb = temp_dir.join("temp_replacement.fsb");
    let temp_wav = temp_dir.join("temp_resampled.wav");
    let audio_path_abs = audio_path.canonicalize()?;
    let audio_str = audio_path_abs.to_string_lossy();
    let audio_clean = audio_str.strip_prefix(r"\\?\").unwrap_or(&audio_str);
    let temp_fsb_str = temp_fsb.to_string_lossy();
    let temp_fsb_clean = temp_fsb_str.strip_prefix(r"\\?\").unwrap_or(&temp_fsb_str);
    let temp_wav_str = temp_wav.to_string_lossy();
    let temp_wav_clean = temp_wav_str.strip_prefix(r"\\?\").unwrap_or(&temp_wav_str);

    let ffmpeg = find_ffmpeg();
    let (encode_path, did_resample) = if let Some(ref ff) = ffmpeg {
        let mut filters = Vec::new();
        if let Some(f) = settings.to_ffmpeg_filter() { filters.push(f); }
        filters.push(format!("aresample={}:ochl={}", target_freq, if target_channels == 1 { "mono" } else { "stereo" }));

        let output = Command::new(ff)
            .args(["-y", "-i", audio_clean, "-af", &filters.join(","), "-ar", &target_freq.to_string(), "-ac", &target_channels.to_string(), temp_wav_clean])
            .output();

        if output.map(|o| o.status.success()).unwrap_or(false) {
            (temp_wav_clean, true)
        } else {
            (audio_clean, false)
        }
    } else {
        (audio_clean, false)
    };

    let output = Command::new(fsbankcl_path)
        .current_dir(fsbankcl_dir)
        .args(["-format", "vorbis", "-quality", "50", "-o", temp_fsb_clean, encode_path])
        .output()?;

    if !output.status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("fsbankcl failed: {}", String::from_utf8_lossy(&output.stderr))));
    }

    let new_bank = FsbBank::load(&temp_fsb)?;
    if new_bank.samples.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "fsbankcl produced empty FSB"));
    }

    let new_data = new_bank.sample_data(0)?.to_vec();
    let new_sample = &new_bank.samples[0];

    if let Some(new_crc) = new_sample.vorbis_crc {
        let mismatch = new_sample.frequency != target_freq || new_sample.channels != target_channels;
        if mismatch && !did_resample {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Format mismatch - install FFmpeg"));
        }
        if get_vorbis_setup_header(new_crc).is_none() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Unsupported CRC 0x{:08X}", new_crc)));
        }
    }

    let old_size = bank.samples[sample_index].data_size as usize;
    let old_offset = bank.samples[sample_index].data_offset as usize;
    let new_size = new_data.len();
    let size_diff = new_size as i64 - old_size as i64;

    let mut new_bank_data = Vec::new();
    new_bank_data.extend_from_slice(&bank.data[..old_offset]);
    new_bank_data.extend_from_slice(&new_data);
    new_bank_data.extend_from_slice(&bank.data[old_offset + old_size..]);

    for s in &mut bank.samples {
        if s.index > sample_index {
            s.data_offset = (s.data_offset as i64 + size_diff) as u64;
        }
    }

    bank.samples[sample_index].data_size = new_size as u64;
    bank.samples[sample_index].frequency = new_sample.frequency;
    bank.samples[sample_index].channels = new_sample.channels;
    bank.samples[sample_index].samples = new_sample.samples;
    bank.samples[sample_index].vorbis_crc = new_sample.vorbis_crc;
    bank.samples[sample_index].vorbis_seek_table = new_sample.vorbis_seek_table.clone();
    bank.data_size = (bank.data_size as i64 + size_diff) as u32;
    bank.data = new_bank_data;

    let _ = std::fs::remove_file(&temp_fsb);
    let _ = std::fs::remove_file(&temp_wav);
    Ok(())
}

fn find_ffmpeg() -> Option<std::path::PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let local = dir.join("ffmpeg.exe");
            if local.exists() { return Some(local); }
        }
    }
    if Command::new("ffmpeg").arg("-version").output().is_ok() {
        return Some(std::path::PathBuf::from("ffmpeg"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let winget = std::path::Path::new(&local).join("Microsoft/WinGet/Packages");
        if let Ok(entries) = std::fs::read_dir(&winget) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(subs) = std::fs::read_dir(&path) {
                        for sub in subs.flatten() {
                            let deep = sub.path().join("bin/ffmpeg.exe");
                            if deep.exists() { return Some(deep); }
                        }
                    }
                }
            }
        }
    }
    None
}

fn generate_vorbis_id_header(sample_rate: u32, channels: u8) -> Vec<u8> {
    let mut h = Vec::with_capacity(30);
    h.push(0x01);
    h.extend_from_slice(b"vorbis");
    h.extend_from_slice(&0u32.to_le_bytes());
    h.push(channels);
    h.extend_from_slice(&sample_rate.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.push(0xB8);
    h.push(0x01);
    h
}

fn generate_vorbis_comment_header() -> Vec<u8> {
    let mut h = Vec::new();
    h.push(0x03);
    h.extend_from_slice(b"vorbis");
    let vendor = b"CUMS";
    h.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    h.extend_from_slice(vendor);
    h.extend_from_slice(&0u32.to_le_bytes());
    h.push(0x01);
    h
}

fn build_ogg_file(id: &[u8], comment: &[u8], setup: &[u8], raw: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    use ogg::writing::PacketWriter;
    let mut output = Vec::new();
    let serial = 0x12345678u32;
    {
        let mut writer = PacketWriter::new(&mut output);
        writer.write_packet(id.to_vec(), serial, ogg::writing::PacketWriteEndInfo::EndPage, 0)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        writer.write_packet(comment.to_vec(), serial, ogg::writing::PacketWriteEndInfo::NormalPacket, 0)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        writer.write_packet(setup.to_vec(), serial, ogg::writing::PacketWriteEndInfo::EndPage, 0)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let mut cursor = Cursor::new(raw);
        let mut granule = 0u64;
        let mut count = 0u32;

        while cursor.position() < raw.len() as u64 {
            let size = match cursor.read_u16::<LittleEndian>() {
                Ok(s) => s as usize,
                Err(_) => break,
            };
            if size == 0 || cursor.position() as usize + size > raw.len() { break; }
            let mut packet = vec![0u8; size];
            if cursor.read_exact(&mut packet).is_err() { break; }

            granule += 1024;
            count += 1;
            let is_last = cursor.position() as usize >= raw.len() - 2;
            let end_info = if is_last {
                ogg::writing::PacketWriteEndInfo::EndStream
            } else if count % 10 == 0 {
                ogg::writing::PacketWriteEndInfo::EndPage
            } else {
                ogg::writing::PacketWriteEndInfo::NormalPacket
            };
            writer.write_packet(packet, serial, end_info, granule)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        }
    }
    Ok(output)
}
