// The rendered DOM of a view for the SEO check (RB-060-08), run through the `evaluate`
// capability in the plugin's isolated world: the document as the browser holds it now, the
// rendered size of every image-like element, the large CSS background images without text, and
// the visible text of the main content (`innerText`, which leaves out what CSS hides) for the
// parity of the views (RB-060-09).
// It reads only; it neither scrolls (scrolling one view would scroll every synced view) nor
// changes the page. The paths are the ones src/path.rs builds (the shared element path of
// tests/fixtures/element-path/cases.json; web/src/collect-path.test.ts holds this copy to it).
(() => {
  const MAX_HTML = 3 * 1024 * 1024;
  const MAX_IMAGES = 2000;
  const MAX_BACKGROUNDS = 200;
  const MAX_SCANNED = 20000;
  const MIN_BACKGROUND_AREA = 40000;
  const MAX_TEXT = 200000;

  // element-path: begin
  const pathOf = (element) => {
    const steps = [];
    for (let node = element; node && node.nodeType === 1; node = node.parentElement) {
      const name = node.localName;
      const tag = name.toLowerCase();
      const parent = node.parentElement;
      if (!parent) {
        steps.push(tag);
        break;
      }
      const same = Array.prototype.filter.call(parent.children, (child) => child.localName === name);
      steps.push(same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(node) + 1})` : tag);
    }
    return steps.reverse().join(' > ');
  };
  // element-path: end

  const root = document.documentElement;
  let html = root ? root.outerHTML : '';
  const truncated = html.length > MAX_HTML;
  if (truncated) {
    html = html.slice(0, MAX_HTML);
  }

  const images = [];
  for (const element of document.querySelectorAll('img, input[type="image" i], svg, area')) {
    if (images.length >= MAX_IMAGES) {
      break;
    }
    const box = element.getBoundingClientRect();
    images.push({ path: pathOf(element), width: Math.round(box.width), height: Math.round(box.height) });
  }

  const backgrounds = [];
  const all = document.body ? document.body.getElementsByTagName('*') : [];
  for (let index = 0; index < all.length && index < MAX_SCANNED; index += 1) {
    if (backgrounds.length >= MAX_BACKGROUNDS) {
      break;
    }
    const element = all[index];
    const image = getComputedStyle(element).backgroundImage || '';
    const match = /url\(\s*["']?([^"')]*)["']?\s*\)/.exec(image);
    if (!match) {
      continue;
    }
    const box = element.getBoundingClientRect();
    if (box.width * box.height < MIN_BACKGROUND_AREA) {
      continue;
    }
    const text = (element.innerText || '').trim();
    const named = element.getAttribute('aria-label') || element.getAttribute('title')
      || element.getAttribute('aria-labelledby');
    const hidden = element.getAttribute('aria-hidden') === 'true'
      || ['presentation', 'none'].includes(element.getAttribute('role') || '');
    if (text || named || hidden) {
      continue;
    }
    backgrounds.push({
      path: pathOf(element),
      url: match[1].slice(0, 2048),
      width: Math.round(box.width),
      height: Math.round(box.height),
    });
  }

  const main = document.querySelector('main, [role="main"]') || document.body;
  const text = main ? String(main.innerText || '').slice(0, MAX_TEXT) : '';

  return { url: location.href, html, truncated, images, backgrounds, text };
})()
