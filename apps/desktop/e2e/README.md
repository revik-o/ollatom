# Desktop browser E2E tests

These Playwright tests exercise the Angular UI in Chromium. The configured web
server starts automatically for a test run.

Install the browser once:

```bash
npm run e2e:install
```

Run headless tests or Playwright's interactive UI:

```bash
npm run e2e
npm run e2e:ui
```

These tests do not drive the native Tauri window or native commands. Native
shell E2E coverage should use Tauri's WebDriver integration separately.
