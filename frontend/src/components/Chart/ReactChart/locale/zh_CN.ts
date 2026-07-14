import { type Time } from 'lightweight-charts';


let _tzOffsetSec: number | null = null;

function getTzOffsetSec(): number {
  if (_tzOffsetSec === null) {


    _tzOffsetSec = -new Date().getTimezoneOffset() * 60;
  }
  return _tzOffsetSec;
}


export function toLocaleTime(utcSec: number): Time {
  return (utcSec + getTzOffsetSec()) as Time;
}
