import { platformFor } from './platform.js';
const platform = platformFor(navigator.userAgent, navigator.platform, navigator.maxTouchPoints);
const hint = document.querySelector('#platform-hint');
const mac = document.querySelector('#download-mac');
const windows = document.querySelector('#download-windows');
if (platform === 'phone') {
  hint.textContent = 'On your computer, open spoofed1.github.io to download the helper. Keep this iPhone for the next step.';
} else if (platform === 'windows') {
  windows.classList.remove('secondary');
  mac.classList.add('secondary');
  windows.parentElement.prepend(windows);
  hint.textContent = 'Download for Windows, then open the helper.';
} else if (platform === 'mac') {
  hint.textContent = 'Download for Mac, unzip, then open Spoofed Helper.';
} else {
  hint.textContent = 'Use a Mac or Windows PC for this helper. Choose your computer below.';
}
