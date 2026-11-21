# Changes

## 0.1.0

The first version (RB-093-02): in every view, each `<img>` measured at the view's device pixel
ratio - the file the browser took from `srcset` or `<picture>` (`currentSrc`, the `<source>` and
its media), the file's own pixels (decoded again from the cache, else from the srcset
descriptor), the size it is drawn at and the device pixels that needs, `sizes`, `loading`,
`fetchpriority` and whether it is the Largest Contentful Paint element. Findings for
`checks_run` and the findings panel: `oversized` (more than `responsive-images.oversizeFactor`
times the pixels needed, default 1.5), `undersized` (less than
`responsive-images.undersizeFactor`, default 1), `sizes-missing` (a srcset with `w` descriptors
and no `sizes`, on the image or a `<source>`) and `lcp` (the LCP image lazy or without
`fetchpriority="high"`, `responsive-images.checkLcp`), each with the view's density; `oversized`,
`undersized` and `lcp` claim the shared ids `image-oversized`, `image-undersized` and
`lcp-image-discovery` and name the element by Lighthouse's node path. The panel: a tab per view
with its density and problems, a table of its images; hovering an image marks it in every view.
MCP `images_check`, `images_mark`; commands `plugin.responsive-images.check`
(`Ctrl+Shift+Space K N`, also Checks in the page's context menu) and
`plugin.responsive-images.panel` (`Ctrl+Shift+Space P V`). Settings in Checks > Responsive
images.
