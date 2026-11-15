// Swaps changed stylesheets and images in place (RB-030-05): no reload, so JS state, scroll
// position and form values stay. Called with { css: [names], images: [names], allCss: bool };
// names are file names. Returns the number of things swapped.
(function (args) {
  'use strict';
  var stamp = String(Date.now());
  var swapped = 0;

  function name(url, base) {
    try {
      var path = new URL(url, base || document.baseURI).pathname;
      return decodeURIComponent(path.slice(path.lastIndexOf('/') + 1));
    } catch (e) {
      return '';
    }
  }

  function bust(url, base) {
    try {
      var next = new URL(url, base || document.baseURI);
      next.searchParams.set('rb-reload', stamp);
      return next.href;
    } catch (e) {
      return url;
    }
  }

  function wanted(names, url, base) {
    return names.indexOf(name(url, base)) >= 0;
  }

  function swapStylesheets() {
    var links = Array.prototype.slice.call(
      document.querySelectorAll('link[rel~="stylesheet"][href]')
    );
    var hit = args.allCss ? links : links.filter(function (link) {
      return wanted(args.css, link.href);
    });
    // A partial pulled in with @import, or a build output under another name: swap them all.
    if (!hit.length) {
      hit = links;
    }
    hit.forEach(function (link) {
      var next = link.cloneNode(false);
      next.href = bust(link.href);
      // The old sheet stays until the new one has loaded, so nothing flashes unstyled.
      next.addEventListener('load', function () { link.remove(); });
      next.addEventListener('error', function () { next.remove(); });
      link.after(next);
      swapped++;
    });
  }

  function rewriteSrcset(value, base) {
    return value.split(',').map(function (entry) {
      var parts = entry.trim().split(/\s+/);
      if (parts[0] && wanted(args.images, parts[0], base)) {
        parts[0] = bust(parts[0], base);
        swapped++;
      }
      return parts.join(' ');
    }).join(', ');
  }

  function rewriteUrls(value, base) {
    return value.replace(/url\((['"]?)([^'")]+)\1\)/g, function (all, quote, url) {
      if (!wanted(args.images, url, base)) {
        return all;
      }
      swapped++;
      return 'url(' + quote + bust(url, base) + quote + ')';
    });
  }

  function rewriteStyle(style, base) {
    for (var i = 0; i < style.length; i++) {
      var property = style[i];
      var value = style.getPropertyValue(property);
      if (value.indexOf('url(') < 0) {
        continue;
      }
      var next = rewriteUrls(value, base);
      if (next !== value) {
        style.setProperty(property, next, style.getPropertyPriority(property));
      }
    }
  }

  function rewriteRules(rules, base) {
    for (var i = 0; i < rules.length; i++) {
      var rule = rules[i];
      if (rule.style) {
        rewriteStyle(rule.style, base);
      }
      if (rule.cssRules) {
        rewriteRules(rule.cssRules, base);
      }
    }
  }

  function swapImages() {
    document.querySelectorAll('img[src]').forEach(function (img) {
      if (wanted(args.images, img.getAttribute('src'))) {
        img.src = bust(img.getAttribute('src'));
        swapped++;
      }
    });
    document.querySelectorAll('img[srcset], source[srcset]').forEach(function (el) {
      el.srcset = rewriteSrcset(el.getAttribute('srcset'));
    });
    document.querySelectorAll('[style*="url("]').forEach(function (el) {
      rewriteStyle(el.style);
    });
    // CSS url(): rewritten rule by rule, so media queries keep choosing the image.
    Array.prototype.forEach.call(document.styleSheets, function (sheet) {
      var rules;
      try {
        rules = sheet.cssRules;
      } catch (e) {
        return; // another origin's sheet cannot be read
      }
      rewriteRules(rules, sheet.href || document.baseURI);
    });
  }

  if (args.allCss || args.css.length) {
    swapStylesheets();
  }
  if (args.images.length) {
    swapImages();
  }
  return swapped;
})
