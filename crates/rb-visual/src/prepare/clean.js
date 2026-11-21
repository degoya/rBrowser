// The capture preparation (RB-080-12), evaluated in rBrowser's isolated world `rbrowser-capture`
// of the main frame, out of the page's reach. `prepare(options)` hides the noise selectors with
// one style element (`display: none !important`, each selector its own rule, so a broken one
// costs only itself), lifts the scroll lock a banner put on `html`/`body`, pauses CSS animations
// at a fixed point (infinite ones at their start, finite ones just before their end), stops
// transitions and the caret, and pauses every video on its first frame; it answers a token.
// `undo(token)` puts everything back as it was: the times of the animations and videos, the
// playing videos playing again, the inline styles, the style element gone. The selectors come
// in as JSON data, never as code. With `mask` (a bug report, RB-093-04) the password fields -
// also one a page shows in clear through its autocomplete hint, one a show-password toggle turned
// into text (marked `data-rbrowser-was-password` from the first time the script sees it as a
// password, for the rest of the document) and one whose name, id, autocomplete, label or
// placeholder says password, secret, token or PIN (marked for the capture alone) - are painted
// black, text and caret transparent. Defines `__rbCaptureClean` once per world.
(function () {
  'use strict';
  if (globalThis.__rbCaptureClean) {
    return;
  }
  var ATTR = 'data-rbrowser-capture';
  var FREEZE_RULE =
    '*, *::before, *::after { animation-play-state: paused !important; ' +
    'transition: none !important; caret-color: transparent !important; }';
  var MASK_ATTR = 'data-rbrowser-mask';
  var WAS_PASSWORD_ATTR = 'data-rbrowser-was-password';
  var MASK_RULE =
    'input[type="password" i], input[autocomplete~="current-password" i], ' +
    'input[autocomplete~="new-password" i], input[data-rbrowser-was-password], ' +
    'input[data-rbrowser-mask] { background: #000 !important; ' +
    'background-image: none !important; color: transparent !important; ' +
    '-webkit-text-fill-color: transparent !important; text-shadow: none !important; ' +
    'caret-color: transparent !important; }';
  // The attributes that may name a secret field, and the words that do (a word of its own after
  // `camelCase` and `snake_case` are split: `userPassword`, `pin_code`, `otp`, not `shipping`).
  var HINT_ATTRIBUTES = ['name', 'id', 'autocomplete', 'aria-label', 'placeholder'];
  var SECRET_HINT =
    /(^|[^a-z])(pass(word|wd|code|phrase)?|pwd|secret|token|pin(code)?|otp|one-time-code)([^a-z]|$)/;
  // The longest wait for the videos to show their first frame.
  var SEEK_WAIT_MS = 1000;
  var states = new Map();
  var next = 0;

  // Where a running animation is frozen: an infinite one at its start, a finite one a
  // millisecond before its end (its final look, without finishing it).
  function frozenTime(animation) {
    var timing =
      animation.effect && typeof animation.effect.getComputedTiming === 'function'
        ? animation.effect.getComputedTiming()
        : null;
    var end = timing ? Number(timing.endTime) : Infinity;
    return Number.isFinite(end) ? Math.max(0, end - 1) : 0;
  }

  function unlockScroll(state) {
    var elements = [document.documentElement, document.body];
    for (var i = 0; i < elements.length; i += 1) {
      var element = elements[i];
      if (!element || !element.style || state.inline.some(function (entry) {
        return entry.element === element;
      })) {
        continue;
      }
      var computed = getComputedStyle(element);
      if ((computed.overflowY || computed.overflow) !== 'hidden') {
        continue;
      }
      state.inline.push({ element: element, style: element.getAttribute('style') });
      element.style.setProperty('overflow-y', 'visible', 'important');
    }
  }

  function freezeAnimations(sheet, state) {
    var running =
      typeof document.getAnimations === 'function'
        ? document.getAnimations().filter(function (animation) {
            // A transition ends with `transition: none`; it is not frozen.
            return animation.playState === 'running' && !('transitionProperty' in animation);
          })
        : [];
    var times = running.map(function (animation) {
      return animation.currentTime;
    });
    sheet.insertRule(FREEZE_RULE, sheet.cssRules.length);
    running.forEach(function (animation, index) {
      if (animation.playState === 'idle') {
        return;
      }
      try {
        animation.currentTime = frozenTime(animation);
        state.animations.push({ animation: animation, time: times[index] });
      } catch (error) {
        // An animation that refuses a time stays as it is.
      }
    });
  }

  function freezeVideos(state) {
    var seeks = [];
    var videos = document.querySelectorAll('video');
    for (var i = 0; i < videos.length; i += 1) {
      var video = videos[i];
      state.videos.push({ video: video, paused: video.paused, time: video.currentTime });
      try {
        video.pause();
      } catch (error) {
        // Nothing to pause.
      }
      if (video.currentTime === 0) {
        continue;
      }
      var seeked =
        video.readyState >= 1
          ? new Promise(function (done) {
              video.addEventListener('seeked', done, { once: true });
            })
          : null;
      try {
        video.currentTime = 0;
        if (seeked) {
          seeks.push(seeked);
        }
      } catch (error) {
        // A video without a timeline stays where it is.
      }
    }
    if (seeks.length === 0) {
      return Promise.resolve();
    }
    return Promise.race([
      Promise.all(seeks),
      new Promise(function (done) {
        setTimeout(done, SEEK_WAIT_MS);
      }),
    ]);
  }

  function isPassword(element) {
    return (
      element instanceof HTMLInputElement && String(element.type).toLowerCase() === 'password'
    );
  }

  // Marks every password field of the document from now on: one shown now, and one whose type
  // the page changes away from password (a show-password toggle) while this world lives.
  function watchPasswordFields() {
    var fields = document.querySelectorAll('input');
    for (var i = 0; i < fields.length; i += 1) {
      if (isPassword(fields[i])) {
        fields[i].setAttribute(WAS_PASSWORD_ATTR, '');
      }
    }
    if (watchPasswordFields.observer || typeof MutationObserver !== 'function') {
      return;
    }
    watchPasswordFields.observer = new MutationObserver(function (records) {
      for (var j = 0; j < records.length; j += 1) {
        var record = records[j];
        if (
          record.target instanceof HTMLInputElement &&
          String(record.oldValue).toLowerCase() === 'password'
        ) {
          record.target.setAttribute(WAS_PASSWORD_ATTR, '');
        }
      }
    });
    watchPasswordFields.observer.observe(document, {
      subtree: true,
      attributes: true,
      attributeFilter: ['type'],
      attributeOldValue: true,
    });
  }

  function namesSecret(element) {
    for (var i = 0; i < HINT_ATTRIBUTES.length; i += 1) {
      var value = element.getAttribute(HINT_ATTRIBUTES[i]);
      if (value && SECRET_HINT.test(value.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase())) {
        return true;
      }
    }
    return false;
  }

  // Marks the fields that name a secret for this capture; the undo takes the marks off.
  function markSecretFields(state) {
    var fields = document.querySelectorAll('input');
    for (var i = 0; i < fields.length; i += 1) {
      if (!isPassword(fields[i]) && namesSecret(fields[i])) {
        fields[i].setAttribute(MASK_ATTR, '');
        state.marked.push(fields[i]);
      }
    }
  }

  async function prepare(options) {
    next += 1;
    var token = next;
    var state = { style: null, inline: [], animations: [], videos: [], marked: [] };
    states.set(token, state);
    var answer = { token: token, rules: 0, matched: 0, animations: 0, videos: 0 };
    var style = document.createElement('style');
    style.setAttribute(ATTR, String(token));
    (document.head || document.documentElement).appendChild(style);
    state.style = style;
    var sheet = style.sheet;
    if (!sheet) {
      return answer;
    }
    if (options.hide) {
      var selectors = Array.isArray(options.selectors) ? options.selectors : [];
      for (var i = 0; i < selectors.length; i += 1) {
        try {
          sheet.insertRule(selectors[i] + ' { display: none !important; }', sheet.cssRules.length);
          answer.rules += 1;
          answer.matched += document.querySelectorAll(selectors[i]).length;
        } catch (error) {
          // A selector the page cannot parse is skipped.
        }
      }
      if (answer.matched > 0) {
        unlockScroll(state);
      }
    }
    if (options.mask) {
      watchPasswordFields();
      markSecretFields(state);
      try {
        sheet.insertRule(MASK_RULE, sheet.cssRules.length);
      } catch (error) {
        // An engine without the case flag of attribute selectors: the type alone.
        sheet.insertRule(MASK_RULE.replace(/ i\]/g, ']'), sheet.cssRules.length);
      }
    }
    if (options.freeze) {
      freezeAnimations(sheet, state);
      await freezeVideos(state);
    }
    answer.animations = state.animations.length;
    answer.videos = state.videos.length;
    return answer;
  }

  function undo(token) {
    var state = states.get(token);
    states.delete(token);
    if (!state) {
      // The world lost its state: at least the style element and the capture's marks go.
      var left = document.querySelectorAll('style[' + ATTR + '="' + String(Number(token)) + '"]');
      for (var k = 0; k < left.length; k += 1) {
        left[k].remove();
      }
      var marked = document.querySelectorAll('[' + MASK_ATTR + ']');
      for (var m = 0; m < marked.length; m += 1) {
        marked[m].removeAttribute(MASK_ATTR);
      }
      return false;
    }
    state.videos.forEach(function (entry) {
      try {
        if (entry.video.currentTime !== entry.time) {
          entry.video.currentTime = entry.time;
        }
        if (!entry.paused) {
          var playing = entry.video.play();
          if (playing && typeof playing.catch === 'function') {
            playing.catch(function () {});
          }
        }
      } catch (error) {
        // A video that is gone is left.
      }
    });
    state.animations.forEach(function (entry) {
      try {
        entry.animation.currentTime = entry.time;
      } catch (error) {
        // An animation that is gone is left.
      }
    });
    if (state.style) {
      state.style.remove();
    }
    state.marked.forEach(function (element) {
      element.removeAttribute(MASK_ATTR);
    });
    state.inline.reverse().forEach(function (entry) {
      if (entry.style === null) {
        entry.element.removeAttribute('style');
      } else {
        entry.element.setAttribute('style', entry.style);
      }
    });
    return true;
  }

  globalThis.__rbCaptureClean = { prepare: prepare, undo: undo };
})();
