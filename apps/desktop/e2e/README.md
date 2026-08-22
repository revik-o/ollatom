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

## Mocking IPC

Browser tests can install an IPC handler before Angular starts:

```typescript
import { installIpcMock } from '../support/ipc.mock';

test('loads projects', async ({ page }) => {
  await installIpcMock(page, (command) => {
    if (command === 'list_projects') {
      return [{ name: 'Example' }];
    }

    throw new Error(`Unexpected IPC command: ${command}`);
  });
  await page.goto('/');
});
```

Unit tests can either use Tauri's `mockIPC` helper or replace the
`IPC_TRANSPORT` Angular provider.
