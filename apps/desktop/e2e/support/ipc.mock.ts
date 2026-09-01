import type { Page } from '@playwright/test';

export type TauriIpcMockHandler = (
  command: string,
  payload?: unknown,
) => unknown | Promise<unknown>;

type TauriCallback = (callbackPayload: unknown) => unknown;

type TauriInternals = {
  callbacks?: Map<number, TauriCallback>;
  invoke?: (command: string, payload?: unknown) => Promise<unknown>;
  runCallback?: (callbackIdentifier: number, callbackPayload: unknown) => unknown;
  transformCallback?: (callback?: TauriCallback, once?: boolean) => number;
  unregisterCallback?: (callbackIdentifier: number) => boolean;
};

type TauriEventPluginInternals = {
  unregisterListener?: (eventName: string, callbackIdentifier: number) => void;
};

type TauriMockWindow = {
  __ollatomEmitTauriEvent?: (eventName: string, payload: unknown) => void;
  __ollatomInvokeTauriCommand: TauriIpcMockHandler;
  __TAURI_INTERNALS__?: TauriInternals;
  __TAURI_EVENT_PLUGIN_INTERNALS__?: TauriEventPluginInternals;
};

export async function installTauriIpcMock(page: Page, handler: TauriIpcMockHandler): Promise<void> {
  await page.exposeFunction('__ollatomInvokeTauriCommand', handler);
  await page.addInitScript(() => {
    const mockWindow = window as unknown as TauriMockWindow;
    const tauriInternals = (mockWindow.__TAURI_INTERNALS__ ??= {});
    const eventPluginInternals = (mockWindow.__TAURI_EVENT_PLUGIN_INTERNALS__ ??= {});
    const callbacks = new Map<number, TauriCallback>();
    const eventListenerIdentifiers = new Map<string, Set<number>>();
    let nextCallbackIdentifier = 1;

    function unregisterCallback(callbackIdentifier: number): boolean {
      return callbacks.delete(callbackIdentifier);
    }

    function registerCallback(callback?: TauriCallback, once = false): number {
      const callbackIdentifier = nextCallbackIdentifier;
      nextCallbackIdentifier += 1;

      if (callback) {
        callbacks.set(callbackIdentifier, (callbackPayload) => {
          if (once) {
            unregisterCallback(callbackIdentifier);
          }
          return callback(callbackPayload);
        });
      }

      return callbackIdentifier;
    }

    function runCallback(callbackIdentifier: number, callbackPayload: unknown): unknown {
      return callbacks.get(callbackIdentifier)?.(callbackPayload);
    }

    function registerEventListener(eventName: string, callbackIdentifier: number): number {
      const callbackIdentifiers = eventListenerIdentifiers.get(eventName) ?? new Set<number>();
      callbackIdentifiers.add(callbackIdentifier);
      eventListenerIdentifiers.set(eventName, callbackIdentifiers);
      return callbackIdentifier;
    }

    function unregisterEventListener(eventName: string, callbackIdentifier: number): void {
      unregisterCallback(callbackIdentifier);
      const callbackIdentifiers = eventListenerIdentifiers.get(eventName);
      callbackIdentifiers?.delete(callbackIdentifier);
      if (callbackIdentifiers?.size === 0) {
        eventListenerIdentifiers.delete(eventName);
      }
    }

    function emitTauriEvent(eventName: string, payload: unknown): void {
      const callbackIdentifiers = eventListenerIdentifiers.get(eventName) ?? [];
      for (const callbackIdentifier of callbackIdentifiers) {
        runCallback(callbackIdentifier, {
          event: eventName,
          id: callbackIdentifier,
          payload,
        });
      }
    }

    async function invoke(command: string, payload?: unknown): Promise<unknown> {
      if (command === 'plugin:event|listen') {
        const listener = payload as { event: string; handler: number };
        return registerEventListener(listener.event, listener.handler);
      }

      if (command === 'plugin:event|unlisten') {
        const listener = payload as { event: string; eventId: number };
        unregisterEventListener(listener.event, listener.eventId);
        return null;
      }

      return mockWindow.__ollatomInvokeTauriCommand(command, payload);
    }

    Object.assign(tauriInternals, {
      callbacks,
      invoke,
      runCallback,
      transformCallback: registerCallback,
      unregisterCallback,
    });
    eventPluginInternals.unregisterListener = unregisterEventListener;
    mockWindow.__ollatomEmitTauriEvent = emitTauriEvent;
  });
}

export async function emitMockTauriEvent(
  page: Page,
  eventName: string,
  payload: unknown,
): Promise<void> {
  await page.evaluate(
    ({ emittedEventName, emittedPayload }) => {
      const mockWindow = window as unknown as TauriMockWindow;
      const emitTauriEvent = mockWindow.__ollatomEmitTauriEvent;
      if (!emitTauriEvent) {
        throw new Error('The Tauri IPC mock has not been installed');
      }
      emitTauriEvent(emittedEventName, emittedPayload);
    },
    { emittedEventName: eventName, emittedPayload: payload },
  );
}
