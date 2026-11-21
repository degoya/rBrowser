// The page script of Live CSS (RB-094-04): it runs in the plugin's isolated world of every view
// at document-start (docs/plugins.md, "Page scripts"). It keeps one constructed stylesheet among
// the document's adopted ones - the DOM is shared with the page, this script and its sheet object
// are not - and fills it with the text the component answers or posts. `replaceSync` takes the
// text as CSS only, so nothing of it ever reaches the HTML parser; an empty text takes the sheet
// out again. A scoped block arrives inside its media query, so every view applies it by its own
// width. The top frame only: the draft is written for the page, not for its iframes.
if (window === window.top) {
  const sheet = new CSSStyleSheet();
  let stopped = false;
  // Counts the component's posts, so an answer asked for before one of them cannot undo it.
  let generation = 0;

  /** @param {unknown} css The sheet's text; the sheet goes last among the adopted ones. */
  const apply = (css) => {
    if (stopped || typeof css !== 'string') return;
    const adopted = document.adoptedStyleSheets;
    const others = adopted.filter((other) => other !== sheet);
    if (css === '') {
      if (others.length !== adopted.length) document.adoptedStyleSheets = others;
      return;
    }
    sheet.replaceSync(css);
    document.adoptedStyleSheets = [...others, sheet];
  };

  /** @param {'start' | 'refresh'} reason A new document, or the component asked for it. */
  const ask = (reason) => {
    const asked = generation;
    rbPlugin.send({ op: 'sheet', reason }).then(
      (answer) => {
        if (asked === generation && answer && typeof answer === 'object') apply(answer.css);
      },
      (error) => rbPlugin.log('warning', error),
    );
  };

  rbPlugin.on((message) => {
    if (!message || typeof message !== 'object') return;
    if (typeof message.css === 'string') {
      generation += 1;
      apply(message.css);
    } else if (message.refresh === true) {
      generation += 1;
      ask('refresh');
    }
  });
  rbPlugin.onStop(() => {
    apply('');
    stopped = true;
  });
  ask('start');
}
