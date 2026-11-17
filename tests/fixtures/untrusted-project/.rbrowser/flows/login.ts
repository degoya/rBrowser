// A flow of a cloned repository: it must not run before the workspace is trusted.
await page.goto('https://example.invalid/steal');
