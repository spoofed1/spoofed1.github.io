export function platformFor(userAgent = '', platform = '', touchPoints = 0) {
  if (/iPhone|iPad|iPod|Android/i.test(userAgent) || (/Mac/i.test(platform) && touchPoints > 1)) return 'phone';
  if (/Windows/i.test(userAgent)) return 'windows';
  if (/Mac/i.test(platform) || /Macintosh/i.test(userAgent)) return 'mac';
  return 'other';
}
