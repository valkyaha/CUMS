#[derive(Debug, Clone)]
pub struct VorbisHeaders {
    pub id_header: Vec<u8>,
    pub comment_header: Vec<u8>,
    pub setup_header: Vec<u8>,
}

pub fn generate_id_header(sample_rate: u32, channels: u8) -> Vec<u8> {
    let mut header = Vec::with_capacity(30);
    header.push(0x01);
    header.extend_from_slice(b"vorbis");
    header.extend_from_slice(&0u32.to_le_bytes());
    header.push(channels);
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header.push(0xB8);
    header.push(0x01);
    header
}

pub fn generate_comment_header() -> Vec<u8> {
    let mut header = Vec::new();
    header.push(0x03);
    header.extend_from_slice(b"vorbis");
    let vendor = b"CUMS";
    header.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    header.extend_from_slice(vendor);
    header.extend_from_slice(&0u32.to_le_bytes());
    header.push(0x01);
    header
}

pub struct VorbisPacketIterator<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> VorbisPacketIterator<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        VorbisPacketIterator { data, position: 0 }
    }
}

impl<'a> Iterator for VorbisPacketIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.position + 2 > self.data.len() { return None; }

        let size = u16::from_le_bytes([
            self.data[self.position],
            self.data[self.position + 1],
        ]) as usize;

        self.position += 2;

        if size == 0 || self.position + size > self.data.len() { return None; }

        let packet = &self.data[self.position..self.position + size];
        self.position += size;
        Some(packet)
    }
}

pub fn build_ogg_file(
    headers: &VorbisHeaders,
    raw_data: &[u8],
    _sample_count: u64,
) -> Result<Vec<u8>, String> {
    use ogg::writing::PacketWriter;

    let mut output = Vec::new();
    let serial = 0x12345678u32;

    {
        let mut writer = PacketWriter::new(&mut output);

        writer.write_packet(
            headers.id_header.clone(), serial,
            ogg::writing::PacketWriteEndInfo::EndPage, 0,
        ).map_err(|e| format!("Failed to write id header: {}", e))?;

        writer.write_packet(
            headers.comment_header.clone(), serial,
            ogg::writing::PacketWriteEndInfo::NormalPacket, 0,
        ).map_err(|e| format!("Failed to write comment header: {}", e))?;

        writer.write_packet(
            headers.setup_header.clone(), serial,
            ogg::writing::PacketWriteEndInfo::EndPage, 0,
        ).map_err(|e| format!("Failed to write setup header: {}", e))?;

        let mut granule_pos = 0u64;
        let mut packet_count = 0u32;
        let packets: Vec<_> = VorbisPacketIterator::new(raw_data).collect();
        let total_packets = packets.len();

        for (i, packet) in packets.into_iter().enumerate() {
            granule_pos += 1024;
            packet_count += 1;

            let is_last = i == total_packets - 1;
            let end_info = if is_last {
                ogg::writing::PacketWriteEndInfo::EndStream
            } else if packet_count % 10 == 0 {
                ogg::writing::PacketWriteEndInfo::EndPage
            } else {
                ogg::writing::PacketWriteEndInfo::NormalPacket
            };

            writer.write_packet(packet.to_vec(), serial, end_info, granule_pos)
                .map_err(|e| format!("Failed to write audio packet: {}", e))?;
        }
    }

    Ok(output)
}

pub fn get_vorbis_info(_data: &[u8]) -> Option<(u32, u32)> {
    None
}

pub fn has_valid_vorbis_packets(data: &[u8]) -> bool {
    if data.len() < 2 { return false; }
    let size = u16::from_le_bytes([data[0], data[1]]) as usize;
    size > 0 && size + 2 <= data.len()
}
