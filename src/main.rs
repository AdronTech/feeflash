use clap::Parser;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use feeflash::bootloader::{BOOTLOADER_MAGIC, send_firmware_file, wait_for_bootloader_magic_ack};
use feeflash::dynamixel::{PING_TIMEOUT_MS, scan_ids, send_ping, send_reboot};

#[derive(Parser, Debug)]
#[command(name = "feeflash", about = "Feetech Servo bootloader client")]
struct Args {
    /// Firmware file path
    #[arg(value_name = "FIRMWARE", default_value = "firmware.bin")]
    firmware: String,

    /// Device ID (0..=253). If omitted, auto-scan all IDs.
    #[arg(long, value_name = "ID", env = "FEEFLASH_ID")]
    id: Option<u8>,

    /// Recovery mode: repeatedly send magic and wait for ACK.
    #[arg(long, env = "FEEFLASH_RECOVERY")]
    recovery: bool,

    /// Serial port path
    #[arg(
        long,
        value_name = "PORT",
        env = "FEEFLASH_PORT",
        default_value = "/dev/ttyACM0"
    )]
    port: String,

    /// Initial baud rate (for normal ping/reboot flow)
    #[arg(
        long,
        value_name = "BAUD",
        env = "FEEFLASH_BAUD",
        default_value_t = 1_000_000u32
    )]
    baud: u32,
    // Timeouts are hardcoded; no user configuration needed.
}

fn main() {
    // let ports = serialport::available_ports().expect("No ports found!");
    // for p in ports {
    //     println!("{}", p.port_name);
    // }

    let args = Args::parse();
    let firmware_path = args.firmware.clone();
    let maybe_id = args.id;
    let recovery = args.recovery;

    let normal_timeout = Duration::from_secs(10);

    let mut port = serialport::new(&args.port, args.baud)
        .timeout(normal_timeout)
        .open()
        .expect("Failed to open port");

    if recovery {
        // Recovery: skip ping/reboot. Assume user will power cycle.
        println!("Recovery mode enabled: skipping ping/reboot.");
        println!("Setting baud rate to 500_000...");
        port.set_baud_rate(500_000)
            .expect("Not able to set baudrate to 500_000");

        // Spam magic and wait for ACK.
        let interval = Duration::from_millis(100);
        wait_for_bootloader_magic_ack(&mut *port, interval, None)
            .expect("Failed to receive bootloader ACK in recovery mode");
    } else {
        // Determine device ID:
        // - If user provided --id, use it and require ping to succeed.
        // - Otherwise, scan all IDs and require a single match.
        let device_id = if let Some(id) = maybe_id {
            // Use a short timeout while probing a specific ID.
            port.set_timeout(Duration::from_millis(PING_TIMEOUT_MS))
                .expect("Failed to set ping timeout");
            println!("Pinging device id {}...", id);
            let ping_resp = send_ping(&mut *port, id).expect("Ping failed!");
            println!("Ping response received ({} bytes)", ping_resp.len());
            println!("Response bytes: {:02X?}", ping_resp);
            id
        } else {
            println!("No --id provided. Scanning all IDs (0..=253)...");
            let found = scan_ids(&mut *port).expect("ID scan failed");

            match found.len() {
                0 => {
                    eprintln!("No devices responded to ping. Please check wiring or use --id.");
                    std::process::exit(1);
                }
                1 => {
                    let id = found[0];
                    println!("Found single device with id {}. Using this ID.", id);
                    id
                }
                _ => {
                    eprintln!("Multiple devices found: {:?}", found);
                    eprintln!(
                        "Please re-run with --id <one of: {}>",
                        found
                            .iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    std::process::exit(1);
                }
            }
        };

        // Restore the normal timeout for the rest of the protocol.
        port.set_timeout(normal_timeout)
            .expect("Failed to restore normal timeout");

        // FF FF 01 02 08 F4
        println!("Rebooting device id {} into bootloader...", device_id);
        send_reboot(&mut *port, device_id).expect("Reboot failed!");

        println!("Setting baud rate to 500_000...");
        port.set_baud_rate(500_000)
            .expect("Not able to set baudrate to 500_000");

        // sleep to allow the device to reboot
        println!("Sleeping for 400ms to allow device to reboot...");
        std::thread::sleep(Duration::from_millis(400));

        println!("Sending magic sequence to enter bootloader...");
        // magic sequence "1fBVA"
        port.write(BOOTLOADER_MAGIC)
            .expect("Failed to write magic sequence");

        let mut buf: [u8; 1024] = [0; 1024];
        let read_bytes = port.read(&mut buf).expect("Failed to read from port");

        if read_bytes != 1 {
            panic!("Expected to read 1 byte, got {}", read_bytes);
        }

        if buf[0] != 0x06 {
            panic!("Expected to read byte 0x06, got 0x{:02X}", buf[0]);
        }

        println!("Bootloader acknowledged magic with 0x06");
    }

    // After magic ACK, avoid re-setting baud or extra delay; go straight to init.

    // At this point, bootloader has acknowledged magic (either via recovery loop or normal flow).
    let mut buf: [u8; 1024] = [0; 1024];

    // Tell the bootloader to initialize by sending 0x01 and
    // wait for another 0x06 before starting firmware transfer.
    println!("Sending init byte 0x01 to bootloader...");
    port.write(&[0x01]).expect("Failed to write init byte 0x01");

    let read_bytes = port
        .read(&mut buf)
        .expect("Failed to read init ACK from port");

    if read_bytes != 1 {
        panic!("Expected to read 1 byte for init ACK, got {}", read_bytes);
    }

    if buf[0] != 0x06 {
        panic!(
            "Expected to read byte 0x06 after init, got 0x{:02X}",
            buf[0]
        );
    }

    println!("Bootloader acknowledged init with 0x06");

    // Handshake is complete at this point. Now send the firmware frames.

    println!("Sending firmware from '{}'...", firmware_path);

    send_firmware_file(&mut *port, Path::new(&firmware_path)).expect("Failed to send firmware");
}

// Tests moved into library modules: see `frame` and `dynamixel`.
