# Spoofed setup website

Download page for https://spoofed1.github.io/, with a native desktop helper for Mac and Windows. Static GitHub Pages hosting; no account, analytics, credential upload, or browser USB requirement.

## Development

`node --test tests/*.test.js` checks download/platform selection and the retained legacy USB transport tests. Serve `site/` with a static HTTP server to preview. The deployed page only needs `index.html`, `style.css`, `app.js`, `platform.js`, and notices.

The desktop helper source, packaging, protocol, and hardware acceptance checklist are in `helper/README.md`. The old `engine/` source and transport tests are retained as an experimental reference; the website no longer loads that engine.

## Release

Push the helper source/workflow and a `helper-v<version>` tag. `.github/workflows/helper-release.yml` builds/tests native Mac and Windows binaries and creates GitHub release assets with checksums. After verifying that release, publish the download page through `.github/workflows/pages.yml`. The page's stable `releases/latest/download` URLs must match the packaged asset names.

Never include device credentials in this repository or release. `helper/resources/Info.plist` is app bundle metadata, not a pairing file.

## App requirement

The iPhone app must include `USBPairingInbox` and have file sharing enabled. On the phone, open Setup → Receive from Computer before starting in the desktop helper. The helper sends a bounded nonce-bound plist over House Arrest/AFC. Updated apps return acceptance through `spoofed-setup-result.json`; older compatible receivers require users to check the phone manually. Physical device acceptance is still needed before removing the Preview label.
