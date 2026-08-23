import { TestBed } from '@angular/core/testing';
import { IpcService } from '../ipc/ipc.service';
import { ApplicationConfigService } from './application-config.service';

describe('ApplicationConfigService', () => {
  it('reads and adds configuration properties through IPC', async () => {
    const getApplicationConfigValueByKey = vi.fn().mockResolvedValue('en');
    const setApplicationConfigValue = vi.fn().mockResolvedValue('success');
    TestBed.configureTestingModule({
      providers: [
        {
          provide: IpcService,
          useValue: { getApplicationConfigValueByKey, setApplicationConfigValue },
        },
      ],
    });
    const service = TestBed.inject(ApplicationConfigService);

    await expect(service.readProperty('app.language')).resolves.toBe('en');
    await expect(service.addProperty('app.language', 'ua')).resolves.toBe('success');
    expect(getApplicationConfigValueByKey).toHaveBeenCalledWith('app.language');
    expect(setApplicationConfigValue).toHaveBeenCalledWith('app.language', 'ua');
  });

  it('persists configuration updates in invocation order', async () => {
    let finishFirstUpdate: (status: 'success') => void = () => undefined;
    const setApplicationConfigValue = vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise<'success'>((resolve) => {
            finishFirstUpdate = resolve;
          }),
      )
      .mockResolvedValueOnce('success');
    TestBed.configureTestingModule({
      providers: [
        {
          provide: IpcService,
          useValue: { setApplicationConfigValue },
        },
      ],
    });
    const service = TestBed.inject(ApplicationConfigService);

    const firstUpdate = service.addProperty('app.language', 'ru');
    const secondUpdate = service.addProperty('app.language', 'ua');
    await Promise.resolve();

    expect(setApplicationConfigValue).toHaveBeenCalledTimes(1);
    finishFirstUpdate('success');
    await firstUpdate;
    await secondUpdate;

    expect(setApplicationConfigValue.mock.calls).toEqual([
      ['app.language', 'ru'],
      ['app.language', 'ua'],
    ]);
  });
});
