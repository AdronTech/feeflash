use std::io;
use std::time::Duration;

pub const PING_TIMEOUT_MS: u64 = 100;
pub const SCAN_TIMEOUT_MS: u64 = 30;

/// Build a Dynamixel v1-style packet for instructions like Ping or Reboot.
pub fn build_dyn_packet(id: u8, instruction: u8, params: &[u8]) -> Vec<u8> {
    let length = (params.len() as u8).saturating_add(2); // instruction + checksum
    let mut packet = Vec::with_capacity(4 + params.len());
    packet.push(0xFF);
    packet.push(0xFF);
    packet.push(id);
    packet.push(length);
    packet.push(instruction);
    packet.extend_from_slice(params);

    let sum: u16 = packet.iter().skip(2).map(|&b| b as u16).sum();
    let checksum = (!sum & 0xFF) as u8;
    packet.push(checksum);
    packet
}

pub fn send_ping(port: &mut dyn serialport::SerialPort, id: u8) -> io::Result<Vec<u8>> {
    let packet = build_dyn_packet(id, 0x01, &[]);
    port.write_all(&packet)?;
    port.flush()?;

    let mut ping_buf: [u8; 1024] = [0; 1024];
    let ping_read_bytes = match port.read(&mut ping_buf) {
        Ok(n) => n,
        Err(e) if e.kind() == io::ErrorKind::TimedOut => {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "Ping timed out"));
        }
        Err(e) => return Err(e),
    };

    if ping_read_bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "No ping response received",
        ));
    }

    Ok(ping_buf[..ping_read_bytes].to_vec())
}

pub fn send_reboot(port: &mut dyn serialport::SerialPort, id: u8) -> io::Result<()> {
    let packet = build_dyn_packet(id, 0x08, &[]);
    port.write_all(&packet)?;
    port.flush()?;
    Ok(())
}

pub fn scan_ids(port: &mut dyn serialport::SerialPort) -> io::Result<Vec<u8>> {
    // Use a short timeout to keep scanning quick.
    port.set_timeout(Duration::from_millis(SCAN_TIMEOUT_MS))?;

    let mut found: Vec<u8> = Vec::new();
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    // Scan valid unicast IDs only (0..=253). Exclude 254 (0xFE) which is broadcast.
    let start_id: u8 = 0;
    let end_id: u8 = 0xFD; // 253
    let total: u16 = (end_id - start_id + 1) as u16;

    use std::io::Write as _;

    for (idx, id) in (start_id..=end_id).enumerate() {
        if send_ping(port, id).is_ok() {
            found.push(id);
        }

        let current = (idx as u16) + 1;
        write!(
            handle,
            "\x1b[2K\rScanning IDs ({:3}/{:3}) found: {}",
            current,
            total,
            found.len()
        )?;
        handle.flush()?;
    }

    writeln!(handle)?;

    if !found.is_empty() {
        println!("Responding IDs: {:?}", found);
    }

    // Restore to a generous timeout for the rest of the protocol.
    port.set_timeout(Duration::from_secs(10))?;

    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dyn_packet_checksum_matches_examples() {
        // Ping example from original hardcoded packet: FF FF 01 02 01 FB
        let pkt = build_dyn_packet(0x01, 0x01, &[]);
        assert_eq!(pkt, vec![0xFF, 0xFF, 0x01, 0x02, 0x01, 0xFB]);

        // Reboot example: FF FF 01 02 08 F4
        let pkt = build_dyn_packet(0x01, 0x08, &[]);
        assert_eq!(pkt, vec![0xFF, 0xFF, 0x01, 0x02, 0x08, 0xF4]);
    }
}
