# FeeFlash: Feetech Servo Bootloader Client

A small Rust CLI to flash Feetech Servos:
- Ping and Reboot via Dynamixel v1 frames
- Enter bootloader and send firmware in fixed-size frames
- Recovery mode for bricked devices: repeatedly send magic sequence so you can power-cycle and catch the 800ms bootloader window

## Requirements
- Rust toolchain (stable)
- Linux serial device available
- Access permissions to the serial device (e.g., user in `dialout` group)

The project uses `serialport` crate and targets Rust edition 2024.

## Build
```bash
cargo build --release
```

## Usage

### Quick start
```bash
cargo run --release -- path/to/firmware.bin --port /dev/ttyUSB0 --baud 500000
```
If installed via `cargo install`:
```bash
feeflash --port /dev/ttyUSB0 --baud 500000 path/to/firmware.bin
```
- Tries ID `1` by default; if that fails, scans IDs `1..=254` and prints a compact progress line: `Scanning IDs (x/y) found: N`.
- If `--id` is omitted, it auto-scans IDs `1..=254` and prints a compact progress line: `Scanning IDs (x/y) found: N`.
- If exactly one ID responds it proceeds; if multiple respond it lists them and requires rerun with `--id`.

### CLI options
```bash
cargo run --release -- \
  --port /dev/ttyUSB0 \
  --baud 500000 \
  --id 2 \
  path/to/firmware.bin
```
- `--port`: serial device path (e.g., `/dev/ttyUSB0`, `/dev/ttyACM0`).
- `--baud`: baud rate; normal mode switches to `500_000` for bootloader.
- `--id`: device ID to target; otherwise auto-scan runs if ID `1` fails.
  

### Environment variables
You can configure options via environment variables instead of flags:
```bash
FEEFLASH_PORT=/dev/ttyUSB0 FEEFLASH_BAUD=500000 FEEFLASH_ID=1 \
cargo run --release -- path/to/firmware.bin
```
- `FEEFLASH_PORT`, `FEEFLASH_BAUD`, `FEEFLASH_ID` map to the corresponding CLI flags.

### Recovery mode (firmware bricked)
```bash
cargo run --release -- --recovery path/to/firmware.bin
```
- Skips Ping/Reboot.
- Sets baud to `500_000` immediately.
- Repeatedly sends the magic string `"1fBVA"` and waits for `0x06` ACK, printing `.` while waiting.
- Use this to manually power the device; the bootloader listens for the magic for ~800ms after boot.
- On ACK, the client sends bootloader init (`0x01`) and then streams firmware frames.
 - `--id` is not required in recovery mode.

## Protocol Flow (normal mode)
1. Ping device (Dynamixel v1 frame)
2. Reboot to bootloader (Dynamixel v1 frame)
3. Set baud to `500_000`, sleep ~400ms
4. Send magic `"1fBVA"` and expect one byte `0x06`
5. Send init byte `0x01` and expect `0x06`
6. Stream firmware frames; stop byte `6` for intermediate frames, `4` for last

## Frame Format
- Total size: 70 bytes
- Layout:
  - `index: u8`
  - `n_index: u8` (bitwise inverse of `index`)
  - `unknown_byte: u8` (currently always `0`)
  - `data: [u8; 64]` (firmware chunk; unused bytes padded with `0xFF`)
  - `checksum_h: u8`
  - `checksum_l: u8`
  - `stop: u8` (`6` for more data, `4` for last frame)
- CRC-16/CCITT parameters:
  - Polynomial `0x1021`, initial value `0x0000`
  - Computed over bytes `0..=63` of the frame: `index`, `n_index`, `unknown_byte`, and the first 61 bytes of `data`, matching the reference implementation
- Device response per frame:
  - `0x06` ACK → success
  - `0x15` NAK → the frame is resent (default retries: up to 5)

## Firmware Streaming
- The client reads the firmware file and sends it in 64-byte chunks per frame.
- `index` starts at `1` and increments per frame (wraps on overflow).
- Last frame uses stop byte `4` to indicate completion; device should boot into firmware.

## Configuration
- Set port and baud via CLI or env (see Usage above).
- Typical Linux device paths: `/dev/ttyUSB0`, `/dev/ttyACM0`.

## Troubleshooting
- "No devices responded to ping":
  - Check wiring and power.
  - Ensure serial permissions (e.g., add your user to `dialout` group).
  - Try `--recovery` to catch bootloader on power cycle.
- Multiple IDs detected: re-run with `--id <one of: ...>`.
- If the magic ACK isn't detected:
  - Ensure baud is `500_000` and you power the device immediately after starting recovery mode.

## Notes
- This client uses Dynamixel v1 packet format for Ping/Reboot: `[0xFF, 0xFF, ID, LENGTH, INSTRUCTION, CHECKSUM]` with `LENGTH = 2` for no parameters.
- Checksum is the bitwise NOT of the sum of bytes starting at `ID`.
- The bootloader handshake and CRC behavior mirror the supplied reference algorithm.
