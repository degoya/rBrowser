function (x, y) {
  // "Inspect element"'s hit test (RB-090-51): the topmost element of the page at (x, y), CSS px of
  // the viewport. rBrowser calls it with `Runtime.callFunctionOn` in its isolated world
  // `rbrowser-inspect` of the view's main frame (crates/rbrowser/src/devtools/inspect.rs), so a
  // page that patches `elementsFromPoint` changes nothing. rBrowser's own elements are passed
  // over: every plugin host - a custom element `rbrowser-*` or one marked `data-rbrowser-plugin` -
  // and what it holds. `elementsFromPoint` retargets a hit inside a closed shadow root to its host,
  // which is what is filtered. From the element found it goes on into an open shadow root and a
  // same-origin frame, as `DOM.getNodeForLocation` does; a closed shadow root of the page and a
  // frame it may not read end there, at their host. `null`: nothing of the page at the point.
  'use strict';
  var MAX_DEPTH = 32;

  function ours(element) {
    for (var node = element; node; node = node.parentElement) {
      var name = node.localName || '';
      if (name.indexOf('rbrowser-') === 0 || node.hasAttribute('data-rbrowser-plugin')) return true;
    }
    return false;
  }

  // The first hit of `scope` itself (a hit outside a shadow root is not its own) that is not ours.
  function topmost(scope, px, py) {
    var found = scope.elementsFromPoint(px, py);
    for (var i = 0; i < found.length; i++) {
      var element = found[i];
      if (element.getRootNode() === scope && !ours(element)) return element;
    }
    return null;
  }

  function frameDocument(element) {
    if (element.localName !== 'iframe' && element.localName !== 'frame') return null;
    try {
      return element.contentDocument;
    } catch (error) {
      return null;
    }
  }

  var found = topmost(document, x, y);
  for (var depth = 0; found && depth < MAX_DEPTH; depth++) {
    var inner = null;
    if (found.shadowRoot) {
      inner = topmost(found.shadowRoot, x, y);
    } else {
      var inside = frameDocument(found);
      if (inside) {
        // The frame's viewport starts inside its border and padding.
        var box = found.getBoundingClientRect();
        var style = found.ownerDocument.defaultView.getComputedStyle(found);
        x -= box.left + found.clientLeft + (parseFloat(style.paddingLeft) || 0);
        y -= box.top + found.clientTop + (parseFloat(style.paddingTop) || 0);
        inner = topmost(inside, x, y);
      }
    }
    if (!inner || inner === found) break;
    found = inner;
  }
  return found;
}
