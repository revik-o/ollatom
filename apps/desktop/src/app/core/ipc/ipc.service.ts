import { inject, Injectable, InjectionToken } from '@angular/core';
import { invoke, type InvokeArgs } from '@tauri-apps/api/core';

export type ApplicationConfigValue = string | number;
export type Status = 'success';

export interface IpcTransport {
  invoke<Result>(command: string, args?: InvokeArgs): Promise<Result>;
}

export const IPC_TRANSPORT = new InjectionToken<IpcTransport>('IPC_TRANSPORT', {
  factory: () => ({ invoke }),
  providedIn: 'root',
});

@Injectable({ providedIn: 'root' })
export class IpcService {
  private readonly transport = inject(IPC_TRANSPORT);

  public invoke<Result>(command: string, args?: InvokeArgs): Promise<Result> {
    return this.transport.invoke<Result>(command, args);
  }

  public getApplicationConfigValueByKey(key: string): Promise<ApplicationConfigValue> {
    return this.invoke<ApplicationConfigValue>('get_application_config_value_by_key', { key });
  }

  public setApplicationConfigValue(key: string, value: ApplicationConfigValue): Promise<Status> {
    return this.invoke<Status>('set_application_config_value', { key, value });
  }
}
