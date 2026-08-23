import { expect, test } from '@playwright/test';
import { installIpcMock } from '../support/ipc.mock';

test('opens the desktop application UI with mocked IPC', async ({ page }) => {
  await installIpcMock(page, (command, payload) => {
    if (
      command === 'get_application_config_value_by_key' &&
      (payload as { key?: string }).key === 'app.language'
    ) {
      return 'en';
    }

    throw new Error(`Unexpected IPC command: ${command}`);
  });
  await page.goto('/');

  const applicationLanguage = await page.evaluate(() => {
    const mockWindow = window as unknown as {
      __TAURI_INTERNALS__: {
        invoke(command: string, payload?: unknown): Promise<unknown>;
      };
    };

    return mockWindow.__TAURI_INTERNALS__.invoke('get_application_config_value_by_key', {
      key: 'app.language',
    });
  });

  expect(applicationLanguage).toBe('en');
});
