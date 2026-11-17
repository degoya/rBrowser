# Network fixture (RB-070-02)

`index.html` loads a `srcset` image (`img/hero-480.svg` in a narrow view, `img/hero-1600.svg` in a
wide one), fetches `api/user.json` with the header `x-fixture: original` and shows its error state
with the time it took, and loads `tracking.js`.

`mocks/api.json` is the mock file of the acceptance: copy it to the project's `.rbrowser/mocks/`
and trust the workspace. Then `GET */api/user.json` answers 500 after 800 ms (the page shows
"could not be loaded (500, >= 800 ms)"), `*/tracking.js` is blocked (the page keeps "Tracking not
loaded."). The first matching rule wins: switch `user-500` off in the panel and the rewrite sets
`x-fixture: rewritten` on the request instead (the fixture server's log shows it).
Without trust nothing of it applies and the panel lists the file as blocked.
