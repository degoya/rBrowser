// One step of the stopped animations (RB-094-07), called with the step in ms in rBrowser's
// isolated world of a view's top document: every animation of the document that has a time -
// CSS animations and transitions as well as Web Animations - moves that far on. With the
// document timeline stopped (`Animation.setPlaybackRate` 0) they stay at the new time; the
// next step moves them on again. Answers how many moved.
(function (ms) {
  'use strict';
  if (typeof document === 'undefined' || typeof document.getAnimations !== 'function') {
    return 0;
  }
  var step = Number(ms);
  if (!Number.isFinite(step) || step <= 0) {
    return 0;
  }
  var moved = 0;
  var animations = document.getAnimations();
  for (var i = 0; i < animations.length; i += 1) {
    var animation = animations[i];
    var time = animation.currentTime;
    // A scroll-driven animation's time is a percentage, not ms: it follows the scroll.
    if (typeof time === 'number' && Number.isFinite(time)) {
      animation.currentTime = time + step;
      moved += 1;
    }
  }
  return moved;
})
