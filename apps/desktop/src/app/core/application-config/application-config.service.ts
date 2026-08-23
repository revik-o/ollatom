import { inject, Injectable } from '@angular/core';
import { type ApplicationConfigValue, IpcService, type Status } from '../ipc/ipc.service';
import { AsyncTaskQueueService } from '../async-task-queue/async-task-queue.service';

@Injectable({ providedIn: 'root' })
export class ApplicationConfigService {
  private readonly ipc = inject(IpcService);
  private readonly taskQueue = new AsyncTaskQueueService();

  public readProperty(key: string): Promise<ApplicationConfigValue> {
    return this.ipc.getApplicationConfigValueByKey(key);
  }

  public addProperty(key: string, value: ApplicationConfigValue): Promise<Status> {
    return this.taskQueue.enqueue(() => this.ipc.setApplicationConfigValue(key, value));
  }
}
