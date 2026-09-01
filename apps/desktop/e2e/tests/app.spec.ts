import { expect, test } from '@playwright/test';
import { emitMockTauriEvent, installTauriIpcMock } from '../support/ipc.mock';
import { WINDOW_APPEARANCE_CHANGED_EVENT_NAME } from '../../src/app/core/ipc/ipc-events';

const INITIAL_STARTUP_SNAPSHOT = {
  backdrop: 'opaque',
  chrome: {
    mode: 'native_standard',
    controls: [],
    titleBarHeight: 0,
    controlsSide: 'right',
    controlsInsetStart: 0,
    controlsInsetEnd: 0,
    scaleFactor: 1,
  },
};

test('opens the desktop application UI with mocked IPC', async ({ page }) => {
  const pageErrors: string[] = [];
  const consoleErrors: string[] = [];
  const invokedCommands: string[] = [];

  page.on('pageerror', (error) => pageErrors.push(error.message));
  page.on('console', (message) => {
    if (message.type() === 'error') {
      consoleErrors.push(message.text());
    }
  });

  await installTauriIpcMock(page, (command, payload) => {
    invokedCommands.push(command);

    switch (command) {
      case 'get_application_config_value_by_key': {
        const configurationRequest = payload as { key?: string } | undefined;
        if (configurationRequest?.key !== 'app.language') {
          throw new Error(`Unexpected application configuration key: ${configurationRequest?.key}`);
        }
        return 'en';
      }
      case 'wait_for_background_ready':
        return INITIAL_STARTUP_SNAPSHOT;
      case 'set_window_interactive_regions':
        return null;
      case 'set_window_appearance':
        return 'opaque';
      default:
        throw new Error(`Unexpected IPC command: ${command}`);
    }
  });

  await page.goto('/');

  await expect(page.locator('app-window-frame')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await expect.poll(() => invokedCommands).toContain('set_window_interactive_regions');

  await emitMockTauriEvent(page, WINDOW_APPEARANCE_CHANGED_EVENT_NAME, {
    ...INITIAL_STARTUP_SNAPSHOT,
    backdrop: 'wayland_blur',
  });
  await expect(page.locator('html')).toHaveAttribute('data-backdrop', 'wayland_blur');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
