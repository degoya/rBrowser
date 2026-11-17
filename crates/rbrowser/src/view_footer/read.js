function () {
  var MAX = 4096;
  var MAX_ROOTS = 512;
  var media = [];
  var seen = Object.create(null);
  var skipped = 0;
  var capped = false;
  var sheets = 0;
  function text(list) {
    return list && list.mediaText ? String(list.mediaText) : '';
  }
  // One media list with the lists around it (its sheet's, the enclosing rules'), outermost first.
  function note(context, own) {
    var chain = context.concat([own]);
    var key = chain.join('\u0001');
    if (seen[key]) return;
    if (media.length >= MAX) {
      capped = true;
      return;
    }
    seen[key] = true;
    media.push(chain);
  }
  function walk(rules, context) {
    for (var i = 0; i < rules.length; i++) {
      var rule = rules[i];
      var own = rule.type === 4 || rule.type === 3 ? text(rule.media) : '';
      var inner = own ? context.concat([own]) : context;
      if (own) note(context, own);
      if (rule.type === 3) {
        try {
          if (rule.styleSheet) walk(rule.styleSheet.cssRules, inner);
        } catch (error) {
          skipped++;
        }
      } else if (rule.cssRules) {
        walk(rule.cssRules, inner);
      }
    }
  }
  function read(list) {
    for (var s = 0; list && s < list.length; s++) {
      var sheet = list[s];
      if (sheet.disabled) continue;
      sheets++;
      var own = text(sheet.media);
      if (own) note([], own);
      try {
        walk(sheet.cssRules, own ? [own] : []);
      } catch (error) {
        skipped++;
      }
    }
  }
  // The document's sheets, the constructed ones, and those of the open shadow roots.
  var roots = [document];
  for (var r = 0; r < roots.length && r < MAX_ROOTS; r++) {
    read(roots[r].styleSheets);
    read(roots[r].adoptedStyleSheets);
    var elements = roots[r].querySelectorAll('*');
    for (var e = 0; e < elements.length && roots.length < MAX_ROOTS; e++) {
      if (elements[e].shadowRoot) roots.push(elements[e].shadowRoot);
    }
  }
  var root = parseFloat(getComputedStyle(document.documentElement).fontSize);
  return {
    url: String(location.href),
    media: media,
    skipped: skipped,
    capped: capped,
    sheets: sheets,
    rootFontSizePx: isFinite(root) && root > 0 ? root : null,
  };
}
