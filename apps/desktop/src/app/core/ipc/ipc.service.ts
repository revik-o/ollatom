import { inject, Injectable, InjectionToken } from '@angular/core';
import { invoke, type InvokeArgs } from '@tauri-apps/api/core';

export interface IpcTransport {
  invoke<Result>(command: string, args?: InvokeArgs): Promise<Result>;
}

export const IPC_TRANSPORT = new InjectionToken<IpcTransport>('IPC_TRANSPORT', {
  factory: () => ({ invoke }),
  providedIn: 'root',
});

/** Low-level boundary between Angular and the native Tauri backend. */
@Injectable({ providedIn: 'root' })
export class IpcService {
  private readonly transport = inject(IPC_TRANSPORT);

  public invoke<Result>(command: string, args?: InvokeArgs): Promise<Result> {
    return this.transport.invoke<Result>(command, args);
  }
}
