"""Bundle installed dependency copyright/license texts without accessing credentials."""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parents[1]
host = next(line.split(': ', 1)[1] for line in subprocess.check_output(['rustc', '-vV'], text=True).splitlines() if line.startswith('host: '))
metadata = json.loads(subprocess.check_output([
    'cargo', 'metadata', '--locked', '--format-version', '1', '--filter-platform', host,
    '--manifest-path', str(root / 'Cargo.toml'),
]))
parts = ['Spoofed Helper — third-party notices\n\nBuilt with idevice and egui/eframe.\n']
for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if package['name'] == 'spoofed-helper':
        continue
    folder = pathlib.Path(package['manifest_path']).parent
    parts.append('\n' + '=' * 72 + '\n' + package['name'] + ' ' + package['version'])
    parts.append('License: ' + (package.get('license') or 'See license text'))
    parts.append('Source: ' + (package.get('repository') or package.get('source') or ''))
    candidates = sorted(p for p in folder.rglob('*') if p.is_file() and (p.name.lower().startswith(('license', 'licence', 'copying')) or p.name == 'OFL.txt'))
    if package.get('license_file'):
        candidates.append(folder / package['license_file'])
    if not candidates:
        candidates = sorted(p for p in folder.parent.glob('LICENSE*') if p.is_file())
    seen = set()
    for path in candidates:
        try:
            content = path.read_text()
        except (UnicodeError, OSError):
            continue
        if content not in seen:
            parts.append(content)
            seen.add(content)
(root / 'resources' / 'THIRD_PARTY_NOTICES.txt').write_text('\n\n'.join(parts))
