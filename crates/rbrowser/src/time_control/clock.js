// The page clock (RB-094-07), registered in a view's main world before the page's own scripts
// (`Page.addScriptToEvaluateOnNewDocument` without a world name) and run at once in the
// document already there. `set(config)` makes `Date` - `new Date()`, `Date()`, `Date.now()` -
// and `Intl.DateTimeFormat`'s `format()` and `formatToParts()` without a date read rBrowser's
// clock: from `origin` (ms since the epoch) or `local` (`[year, month, day, hour, minute,
// second, ms]` in the page's own time zone, emulated or not), plus `offset` ms, running at
// `speed` (0: fixed) from the real time `anchor` on. Every view gets the same configuration, so
// all of them show the same time. Timers, `performance.now()` and `requestAnimationFrame` stay
// real: the page keeps running. `reset()` puts the originals back. Defines its API under
// `Symbol.for('rbrowser.clock')` once per realm; the configuration comes in as JSON data, never
// as code.
(function () {
  'use strict';
  var KEY = Symbol.for('rbrowser.clock');
  if (globalThis[KEY]) {
    return;
  }
  var RealDate = globalThis.Date;
  var proto = RealDate.prototype;
  var dtf = typeof Intl === 'object' && Intl.DateTimeFormat ? Intl.DateTimeFormat.prototype : null;
  var formatGetter = dtf ? Object.getOwnPropertyDescriptor(dtf, 'format') : undefined;
  var formatToParts = dtf ? dtf.formatToParts : undefined;
  var bound = new WeakMap();
  var clock = null;
  var installed = false;

  function realNow() {
    return RealDate.now();
  }

  // The clock's time now, ms since the epoch; the real one without a clock.
  function now() {
    var real = realNow();
    if (clock === null) {
      return real;
    }
    return Math.floor(clock.origin + clock.offset + (real - clock.anchor) * clock.speed);
  }

  function FakeDate() {
    // Called without `new`: the time as text, as `Date()` answers.
    if (!new.target) {
      return new RealDate(now()).toString();
    }
    var args = arguments.length === 0 ? [now()] : Array.prototype.slice.call(arguments);
    return Reflect.construct(RealDate, args, new.target);
  }
  FakeDate.prototype = proto;
  FakeDate.now = function () {
    return now();
  };
  FakeDate.parse = RealDate.parse;
  FakeDate.UTC = RealDate.UTC;
  Object.defineProperty(FakeDate, 'name', { value: 'Date' });
  Object.defineProperty(FakeDate, 'length', { value: 7 });

  // `format` is a getter that answers a function bound to its formatter: the same one each
  // time, as the original does.
  function patchedFormat() {
    var format = bound.get(this);
    if (!format) {
      var original = formatGetter.get.call(this);
      format = function (date) {
        return original(date === undefined ? now() : date);
      };
      bound.set(this, format);
    }
    return format;
  }

  function patchedFormatToParts(date) {
    return formatToParts.call(this, date === undefined ? now() : date);
  }

  function install() {
    if (installed) {
      return;
    }
    installed = true;
    globalThis.Date = FakeDate;
    proto.constructor = FakeDate;
    if (dtf && formatGetter && formatGetter.get) {
      Object.defineProperty(dtf, 'format', {
        configurable: true,
        enumerable: formatGetter.enumerable,
        get: patchedFormat,
      });
    }
    if (dtf && typeof formatToParts === 'function') {
      dtf.formatToParts = patchedFormatToParts;
    }
  }

  function uninstall() {
    if (!installed) {
      return;
    }
    installed = false;
    globalThis.Date = RealDate;
    proto.constructor = RealDate;
    if (dtf && formatGetter) {
      Object.defineProperty(dtf, 'format', formatGetter);
    }
    if (dtf && typeof formatToParts === 'function') {
      dtf.formatToParts = formatToParts;
    }
  }

  // The start in ms since the epoch: a local wall time is read in this page's time zone.
  function originOf(config) {
    var local = config.local;
    if (Array.isArray(local) && local.length === 7) {
      var date = new RealDate(local[0], local[1] - 1, local[2], local[3], local[4], local[5], local[6]);
      // `new Date(y, ...)` reads the years 0 to 99 as 1900 to 1999.
      if (local[0] >= 0 && local[0] < 100) {
        date.setFullYear(local[0], local[1] - 1, local[2]);
      }
      return date.getTime();
    }
    return Number(config.origin);
  }

  // Puts `config` in effect; `false` (and nothing changed) when it is no clock.
  function set(config) {
    if (typeof config !== 'object' || config === null) {
      return false;
    }
    var next = {
      origin: originOf(config),
      offset: config.offset === undefined ? 0 : Number(config.offset),
      anchor: Number(config.anchor),
      speed: Number(config.speed),
    };
    if (
      !Number.isFinite(next.origin) ||
      !Number.isFinite(next.offset) ||
      !Number.isFinite(next.anchor) ||
      !Number.isFinite(next.speed) ||
      next.speed < 0
    ) {
      return false;
    }
    clock = next;
    install();
    return true;
  }

  function reset() {
    clock = null;
    uninstall();
    return true;
  }

  Object.defineProperty(globalThis, KEY, {
    value: Object.freeze({ set: set, reset: reset, now: now }),
    configurable: true,
  });
})();
