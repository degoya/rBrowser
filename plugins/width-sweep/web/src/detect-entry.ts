// The entry of ../src/detect.js: the component evaluates the bundle and then
// `WidthSweepDetect.probe({ width, kinds, reset })` (src/probe.rs) at each width of a sweep, or
// `WidthSweepDetect.hold({ width, ms })` (src/show.rs) for a width shown; the answer goes back as
// JSON.

export { probe } from './detect';
export { hold } from './hold';
