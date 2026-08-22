import type { Page } from '@playwright/test';

export type IpcMockHandler = (command: string, payload?: unknown) => unknown | Promise<unknown>;

/** Installs a Tauri IPC replacement before the application is loaded. */
export async function installIpcMock(page: Page, handler: IpcMockHandler): Promise<void> {
  await page.exposeFunction('__ollatomIpcInvoke', handler);
  await page.addInitScript(() => {
    const mockWindow = window as unknown as {
      __ollatomIpcInvoke: IpcMockHandler;
      __TAURI_INTERNALS__?: Record<string, unknown>;
    };
    const internals = (mockWindow.__TAURI_INTERNALS__ ??= {});

    internals['invoke'] = (command: string, payload?: unknown) =>
      mockWindow.__ollatomIpcInvoke(command, payload);
  });
}
