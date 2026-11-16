function () {
  // The pick's check (RB-090-54): whether `this`, the node the inspect overlay answered for a
  // click (`Overlay.inspectNodeRequested`), is rBrowser's own - a plugin host, a custom element
  // `rbrowser-*` (the layers' `<rbrowser-layers>` among them) or one marked
  // `data-rbrowser-plugin`, or anything inside one. The walk goes up through shadow roots to their
  // hosts, closed ones included: the overlay's hit test sees into them. rBrowser calls it with
  // `Runtime.callFunctionOn` in its isolated world of the main frame
  // (crates/rbrowser/src/capture/pick.rs), so a page that patches the DOM changes nothing.
  'use strict';
  var node = this;
  while (node) {
    if (node.nodeType === 1) {
      var name = node.localName || '';
      if (name.indexOf('rbrowser-') === 0 || node.hasAttribute('data-rbrowser-plugin')) return true;
    }
    // A shadow root (a fragment, type 11) has no parent but its host.
    node = node.parentNode || (node.nodeType === 11 ? node.host : null) || null;
  }
  return false;
}
