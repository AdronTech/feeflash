use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use crate::frame::BootloaderFrame;

pub const BOOTLOADER_MAGIC: &[u8] = b"1fBVA";

pub fn wait_for_bootloader_magic_ack(
    port: &mut dyn serialport::SerialPort,
    interval: Duration,
    max_wait: Option<Duration>,
) -> io::Result<()> {
    println!("Recovery mode: power the device now. Spamming magic...");
    port.set_timeout(interval)?;

    let start = std::time::Instant::now();
    let magic = BOOTLOADER_MAGIC;
    let mut buf = [0u8; 1];

    loop {
        port.write_all(magic)?;
        port.flush()?;

        match port.read(&mut buf) {
            Ok(n) if n == 1 => {
                if buf[0] == 0x06 {
                    println!("\nBootloader ACK received.");
                    return Ok(());
                }
            }
            Err(e) if e.kind() == io::ErrorKind::TimedOut => {
                // Lightweight progress indicator
                print!(".");
                let _ = std::io::stdout().flush();
            }
            Err(e) => return Err(e),
            _ => {}
        }

        if let Some(limit) = max_wait {
            if start.elapsed() >= limit {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Timed out waiting for bootloader magic ACK",
                ));
            }
        }
    }
}

pub fn send_frame_with_retry(
    port: &mut dyn serialport::SerialPort,
    frame_bytes: &[u8; 70],
    max_retries: u8,
) -> io::Result<()> {
    let mut attempt: u8 = 0;

    loop {
        attempt = attempt.wrapping_add(1);

        port.write_all(frame_bytes)?;
        port.flush()?;

        let mut resp = [0u8; 1];
        let read_bytes = port.read(&mut resp)?;

        if read_bytes != 1 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "Expected 1-byte response from bootloader, got {}",
                    read_bytes
                ),
            ));
        }

        match resp[0] {
            0x06 => {
                // ACK
                return Ok(());
            }
            0x15 => {
                // NAK, retry if we still have attempts left
                if attempt > max_retries {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Bootloader NAK after {} attempts", attempt - 1),
                    ));
                }
                eprintln!(
                    "Bootloader NAK, retrying frame (attempt {} / {})",
                    attempt, max_retries
                );
                continue;
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Unexpected bootloader response 0x{other:02X} (expected 0x06 or 0x15)"),
                ));
            }
        }
    }
}

pub fn send_firmware_file(
    port: &mut dyn serialport::SerialPort,
    firmware_path: &Path,
) -> io::Result<()> {
    let data = fs::read(firmware_path)?;

    if data.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Firmware file is empty",
        ));
    }

    let total_chunks = (data.len() + 63) / 64;
    println!(
        "Sending firmware ({} bytes) in {} chunks...",
        data.len(),
        total_chunks
    );

    let mut index: u8 = 1;

    for (chunk_idx, chunk) in data.chunks(64).enumerate() {
        let is_last = (chunk_idx + 1) == total_chunks;

        let mut frame_data = [0xFFu8; 64];
        frame_data[..chunk.len()].copy_from_slice(chunk);

        let frame = BootloaderFrame {
            index,
            unknown_byte: 0,
            data: frame_data,
            is_last,
        };

        let raw = frame.to_bytes();

        println!(
            "Sending frame index={} (chunk {}/{}) , last={}...",
            index,
            chunk_idx + 1,
            total_chunks,
            is_last
        );

        send_frame_with_retry(port, &raw, 5)?;

        index = index.wrapping_add(1);
    }

    println!("Firmware transfer complete.");
    Ok(())
}
