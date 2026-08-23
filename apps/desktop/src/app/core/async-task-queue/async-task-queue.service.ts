export class AsyncTaskQueueService {
  private queue: Promise<void> = Promise.resolve();

  private emptyFunction() {
    return undefined;
  }

  public enqueue<T>(task: () => T | Promise<T>): Promise<T> {
    const taskPromise = this.queue.then(() => task(),);
    this.queue = taskPromise.then(this.emptyFunction, this.emptyFunction);
    return taskPromise;
  }
}
