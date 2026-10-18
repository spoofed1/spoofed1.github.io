# Spoofed USB setup

Static GitHub Pages site for https://spoofed1.github.io/.

Preview implementation: browser-to-phone pairing has not yet been validated on physical hardware. No server, login, analytics, remote credential storage or desktop helper. WebUSB permission and the phone's Trust approval are required. USB interface ownership/driver restrictions can prevent browser access.

## Build

Install the wasm32-unknown-unknown Rust target and wasm-bindgen-cli 0.2.114, then run `sh scripts/build.sh`. The exact idevice revision and Cargo dependencies are pinned. Generated browser assets are committed in `site/pkg` for Pages. `node --test tests/*.test.js` checks the USB stream lifecycle and framing.

## App contract

Install the updated Spoofed app (`com.yashpattekar.myapp`). Enable its Receive from Computer screen before connecting. The app creates a random, ten-minute request nonce in `Documents/spoofed-setup-request.json`. Browser setup opens the app's Documents over House Arrest/AFC, creates a trusted remote pairing using the lockdown remote-pairing service, then writes a plist envelope with `nonce` and `pairing` (Data) to `Documents/spoofed-pairing.partial`. It closes the file and atomically renames it to `Documents/spoofed-pairing.plist`.

Spoofed polls only while the setup screen exists, checks the nonce/expiry/file size/credential structure, installs into backup-excluded protected Application Support storage, and removes transfer files. Closing or cancelling setup invalidates the nonce. The browser reports delivery, not app acceptance or successful simulation. The user checks acceptance on the phone. The first simulation connection verifies trust. Stop simulation before replacing pairing this way.

No credentials are persisted in browser storage or sent over HTTP. Lockdown certificates and RPPairing are held in memory for this session only. JavaScript/WASM memory is not guaranteed to be securely zeroed; closing the tab releases the session. The host trust created on the phone remains until revoked through iOS.

## Physical acceptance

1. Updated app installed, LocalDevVPN available, Developer Mode enabled; open Receive from Computer.
2. Desktop Chrome/Edge: connect an unlocked iPhone over a data cable, select the phone and approve Trust.
3. Confirm site reaches Sent, and app says Pairing received. Confirm temporary Documents files are gone.
4. Disconnect USB, connect LocalDevVPN and Teleport; then Stop Simulation.
5. Exercise cancelled USB chooser, denied Trust, cable removal during setup, no app installed, expired receiver, repeated setup, and a wrong/invalid delivery.
6. Check supported macOS/Windows/Linux USB driver combinations before removing the Preview badge.

Source acknowledgments: site/THIRD_PARTY_NOTICES.md. USB framing reference: rorkai/rork-device revision 3e4fed0b807695f6ac24ead827f1a264eaae0063.
