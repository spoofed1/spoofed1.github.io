import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { platformFor } from '../site/platform.js';
test('offers desktop download without treating iPad desktop mode as a Mac', () => {
  assert.equal(platformFor('Mozilla Macintosh', 'MacIntel', 5), 'phone');
  assert.equal(platformFor('iPhone', 'iPhone', 1), 'phone');
  assert.equal(platformFor('Windows NT 10.0', 'Win32'), 'windows');
  assert.equal(platformFor('Macintosh', 'MacIntel', 0), 'mac');
  assert.equal(platformFor('Linux', 'Linux'), 'other');
});
test('download page works without JavaScript and no longer starts WebUSB', () => {
  const html = readFileSync(new URL('../site/index.html', import.meta.url), 'utf8');
  assert.match(html, /releases\/latest\/download\/Spoofed-Helper-macOS.zip/);
  assert.match(html, /releases\/latest\/download\/Spoofed-Helper-Windows.exe/);
  assert.doesNotMatch(html, /id="connect"|No helper app/);
  const script = readFileSync(new URL('../site/app.js', import.meta.url), 'utf8');
  assert.doesNotMatch(script, /navigator.usb|spoofed_web/);
});
