/// CRC-16/CCITT (poly 0x1021, init 0x0000), bit-by-bit implementation
/// matching the provided C algorithm.
pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0x0000;

    // The original code iterates exactly 64 bytes; here we assume the caller
    // passes the correct slice (length >= 64) and only the first 64 are used.
    for &byte in data.iter().take(64) {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            let msb_set = (crc & 0x8000) != 0;
            crc = (crc & 0x7FFF) << 1;
            if msb_set {
                crc ^= 0x1021;
            }
        }
    }

    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_length_insensitive_beyond_64() {
        let mut data = vec![0u8; 100];
        data[0] = 0x12;
        let c1 = crc16_ccitt(&data[..64]);
        let c2 = crc16_ccitt(&data); // only first 64 used
        assert_eq!(c1, c2);
    }
}
