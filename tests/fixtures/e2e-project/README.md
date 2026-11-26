# The end-to-end project

The project `scripts/e2e-windows.sh` opens for most criteria (RB-099-02, `tests/e2e/README.md`):
three Chromium views (desktop, tablet, mobile at DPR 3) on the fixture server
(`tests/e2e/lib/server.ts`, port 18766), and `www/` watched for hot reload (K6). The run copies
it into its work folder first: K6 writes `www/reload.css` and `www/reload.js` there, never here.
