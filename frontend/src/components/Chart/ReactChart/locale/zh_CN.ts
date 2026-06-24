import { type Time } from 'lightweight-charts';

const CST_OFFSET = 8 * 3600;

export function toLocaleTime(utcSec: number): Time {
  return (utcSec + CST_OFFSET) as Time;
}
