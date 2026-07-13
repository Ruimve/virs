import { type Time } from 'lightweight-charts';

// Lazily compute the user's local timezone offset in seconds.
// This avoids accessing browser APIs at module load time (SSR-safe) and
// automatically adapts to each user's locale instead of hardcoding CST+8.
let _tzOffsetSec: number | null = null;

function getTzOffsetSec(): number {
  if (_tzOffsetSec === null) {
    // getTimezoneOffset() returns minutes difference (UTC - local).
    // For UTC+8 it returns -480, so we negate to get +480 min = +28800 sec.
    _tzOffsetSec = -new Date().getTimezoneOffset() * 60;
  }
  return _tzOffsetSec;
}

/**
 * Convert a UTC timestamp (in seconds) to a lightweight-charts Time value
 * adjusted for the user's local timezone.
 *
 * This shifts the timestamp so that the chart axis displays local time
 * while the underlying Time value remains a valid UTCTimestamp.
 */
export function toLocaleTime(utcSec: number): Time {
  return (utcSec + getTzOffsetSec()) as Time;
}
