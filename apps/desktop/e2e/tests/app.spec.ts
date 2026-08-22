import { expect, test } from '@playwright/test';
import { installIpcMock } from '../support/ipc.mock';

test('opens the desktop application UI with mocked IPC', async ({ page }) => {
  await installIpcMock(page, (command) => {
    if (command === 'health_check') {
      return { ok: true };
    }

    throw new Error(`Unexpected IPC command: ${command}`);
  });
  await page.goto('/');

  const health = await page.evaluate(() => {
    const mockWindow = window as unknown as {
      __TAURI_INTERNALS__: {
        invoke(command: string): Promise<unknown>;
      };
    };

    return mockWindow.__TAURI_INTERNALS__.invoke('health_check');
  });

  expect(health).toEqual({ ok: true });
});
