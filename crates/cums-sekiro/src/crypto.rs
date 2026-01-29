use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes256;

pub const FSB_KEY: &[u8; 32] = b"G0KTrWjS9syqF7vVD6RaVXlFD91gMgkC";

pub fn decrypt_aes_block(data: &mut [u8], key: &[u8; 32]) {
    let cipher = Aes256::new(GenericArray::from_slice(key));
    for chunk in data.chunks_exact_mut(16) {
        let block = GenericArray::from_mut_slice(chunk);
        cipher.decrypt_block(block);
    }
}

pub fn encrypt_aes_block(data: &mut [u8], key: &[u8; 32]) {
    let cipher = Aes256::new(GenericArray::from_slice(key));
    for chunk in data.chunks_exact_mut(16) {
        let block = GenericArray::from_mut_slice(chunk);
        cipher.encrypt_block(block);
    }
}

pub fn decrypt_aes_data(data: &mut [u8], key: &[u8; 32]) {
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let block_count = data.len() / 16;
    for i in 0..block_count {
        let start = i * 16;
        let chunk = &mut data[start..start + 16];
        let block = GenericArray::from_mut_slice(chunk);
        cipher.decrypt_block(block);
    }
}

pub fn encrypt_aes_data(data: &mut [u8], key: &[u8; 32]) {
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let block_count = data.len() / 16;
    for i in 0..block_count {
        let start = i * 16;
        let chunk = &mut data[start..start + 16];
        let block = GenericArray::from_mut_slice(chunk);
        cipher.encrypt_block(block);
    }
}

fn fsbdec_byte(t: u8) -> u8 {
    let t = t as u32;
    ((((((((t & 64) | (t >> 2)) >> 2) | (t & 32)) >> 2) | (t & 16)) >> 1)
        | (((((((t & 2) | (t << 2)) << 2) | (t & 4)) << 2) | (t & 8)) << 1)) as u8
}

fn fsbenc_byte(t: u8) -> u8 {
    static ENCODE_TABLE: std::sync::OnceLock<[u8; 256]> = std::sync::OnceLock::new();
    let table = ENCODE_TABLE.get_or_init(|| {
        let mut tbl = [0u8; 256];
        for i in 0u8..=255 {
            let dec = fsbdec_byte(i);
            tbl[dec as usize] = i;
        }
        tbl
    });
    table[t as usize]
}

pub fn fsbext_decrypt(data: &mut [u8], key: &[u8]) {
    for (i, byte) in data.iter_mut().enumerate() {
        let k = key[i % key.len()];
        *byte = fsbdec_byte(*byte) ^ k;
    }
}

pub fn fsbext_encrypt(data: &mut [u8], key: &[u8]) {
    for (i, byte) in data.iter_mut().enumerate() {
        let k = key[i % key.len()];
        *byte = fsbenc_byte(*byte ^ k);
    }
}
