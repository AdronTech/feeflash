use crate::crc::crc16_ccitt;

pub struct BootloaderFrame {
    pub index: u8,
    pub unknown_byte: u8,
    pub data: [u8; 64],
    pub is_last: bool,
}

impl BootloaderFrame {
    /// Build the 70-byte raw frame expected by the bootloader.
    /// Layout (70 bytes total):
    /// [0]   index
    /// [1]   n_index (bitwise inverse of index)
    /// [2]   unknown_byte
    /// [3..67) data[0..64]
    /// [67]  checksum_h
    /// [68]  checksum_l
    /// [69]  stop (6 for more data, 4 for last frame)
    pub fn to_bytes(&self) -> [u8; 70] {
        let mut frame = [0u8; 70];

        frame[0] = self.index;
        frame[1] = !self.index; // n_index
        frame[2] = self.unknown_byte;
        frame[3..3 + 64].copy_from_slice(&self.data);

        // CRC is calculated over the first 64 bytes of the frame:
        // index, n_index, unknown_byte, data[0..=60]. That corresponds to
        // frame[0..64] (64 bytes total).
        let crc = crc16_ccitt(&frame[0..64]);
        let crc_high = (crc >> 8) as u8;
        let crc_low = (crc & 0xFF) as u8;

        frame[67] = crc_high;
        frame[68] = crc_low;
        frame[69] = if self.is_last { 4 } else { 6 };

        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_has_correct_size_and_inverse_index() {
        let mut data = [0u8; 64];
        data[0] = 0x12;
        data[1] = 0x34;

        let frame = BootloaderFrame {
            index: 0x5A,
            unknown_byte: 0,
            data,
            is_last: false,
        };

        let raw = frame.to_bytes();
        assert_eq!(raw.len(), 70);
        assert_eq!(raw[0], 0x5A);
        assert_eq!(raw[1], !0x5A);
        // Stop byte 6 for non-last frame
        assert_eq!(raw[69], 6);
    }
}
