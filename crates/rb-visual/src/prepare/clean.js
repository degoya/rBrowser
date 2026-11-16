// The capture preparation (RB-080-12), evaluated in rBrowser's isolated world `rbrowser-capture`
// of the main frame, out of the page's reach. `prepare(options)` hides the noise selectors with
// one style element (`display: none !important`, each selector its own rule, so a broken one
// costs only itself), lifts the scroll lock a banner put on `html`/`body`, pauses CSS animations
// at a fixed point (infinite ones at their start, finite ones just before their end), stops
// transitions and the caret, and pauses every video on its first frame; it answers a token.
// `undo(token)` puts everything back as it was: the times of the animations and videos, the
// playing videos playing again, the inline styles, the style element gone. The selectors come
// in as JSON data, never as code. Defines `__rbCaptureClean` once per world.
(function () {
  'use strict';
  if (globalThis.__rbCaptureClean) {
    return;
  }
  var ATTR = 'data-rbrowser-capture';
  var FREEZE_RULE =
    '*, *::before, *::after { animation-play-state: paused !important; ' +
    'transition: none !important; caret-color: transparent !important; }';
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

  async function prepare(options) {
    next += 1;
    var token = next;
    var state = { style: null, inline: [], animations: [], videos: [] };
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
      // The world lost its state: at least the style element goes.
      var left = document.querySelectorAll('style[' + ATTR + '="' + String(Number(token)) + '"]');
      for (var k = 0; k < left.length; k += 1) {
        left[k].remove();
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
