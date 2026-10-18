# Spoofed Helper

A small native Mac/Windows GUI that discovers USB-connected iPhones, uses the Apple USB service to establish trust, creates RPPairing credentials, validates them by reconnecting, and delivers them into Spoofed's open receive session. No embedded browser, server, account, analytics, or persistent remote-pairing copy.

## Build

Rust 1.92+, platform build tools. `cargo test --locked`, then `cargo run --release --locked`. `cargo run --release --locked -- --probe` tests USB discovery without pairing or printing device identifiers.

macOS packaging: install Rust targets `aarch64-apple-darwin` and `x86_64-apple-darwin`, run `python3 scripts/notices.py`, then `bash scripts/package-macos.sh`. Produces a universal `.app` and zip in `dist/`. Signing is ad hoc for the preview, not Apple Developer ID signing or notarization.

Windows: `cargo build --release --locked`; distribute `target/release/spoofed-helper.exe` as `Spoofed-Helper-Windows.exe`. Apple Devices supplies the Apple Mobile Device service/USB driver. No replacement USB driver or administrator mode is used by this helper.

The GitHub release workflow builds/tests both platforms and packages downloads with checksums and notices. The website is deployed only after those assets are published.

## Protocol

Bundle ID: `com.yashpattekar.myapp`. All paths are under House Arrest's `Documents/` root. Requires the iOS app's file-sharing entitlement/configuration and an explicitly opened Receive from Computer screen.

1. Read bounded `spoofed-setup-request.json`; require version 1 and UUID nonce.
2. Create/commit remote pairing with the pinned idevice revision. Re-read the request to reject changed sessions.
3. Write XML plist `{nonce: String, pairing: Data}` to `spoofed-pairing.partial`, close, then rename to `spoofed-pairing.plist`.
4. The app validates, saves to protected backup-excluded storage, removes transfer files, and writes `{nonce: String, accepted: Bool}` to `spoofed-setup-result.json`.
5. Only a receipt with this nonce reports success. After 25 seconds without confirmation, show **sent**, never accepted. Overall setup times out after three minutes. Cancel aborts connections; it cannot undo a delivery already accepted by the phone. Restart receiving to invalidate/clean a cancelled session.

The app removes receipts when setup closes or another session starts. The helper never formats device protocol responses in its UI/logs. Normal Apple lockdown trust is stored through usbmuxd, while RPPairing remains in process memory until delivery. Memory is not guaranteed to be zeroed. Removing the local app file does not revoke device-side trust.

## Physical acceptance still required

On both macOS and Windows: first Trust, existing Trust, denied Trust, locked phone, missing Apple USB service, missing app, two phones/selection, expired/cancelled receiver, cable removal during pairing/transfer, malformed rejection, and confirmed successful transfer. Then disconnect USB, connect LocalDevVPN, teleport and stop on the iPhone. Verify old app receivers show delivery without claiming acceptance.
