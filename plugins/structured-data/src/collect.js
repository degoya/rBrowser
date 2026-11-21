// The structured data panel's collector (RB-090-96): one expression the component runs through
// `evaluate` in the plugin's isolated world. It only reads: the page's URL and its rendered
// document as HTML, so JSON-LD a script added counts too. A document longer than the plugin
// reads is named, not sent.
// SPDX-License-Identifier: GPL-3.0-or-later
(() => {
  const limit = 3500000;
  const html = document.documentElement ? document.documentElement.outerHTML : '';
  return { url: location.href, length: html.length, html: html.length > limit ? null : html };
})()
