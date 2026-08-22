import { TestBed } from '@angular/core/testing';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { IPC_TRANSPORT, IpcService } from './ipc.service';

describe('IpcService', () => {
  afterEach(() => {
    clearMocks();
    TestBed.resetTestingModule();
  });

  it('invokes a Tauri command', async () => {
    mockIPC((command, payload) => ({ command, payload }));
    const service = TestBed.inject(IpcService);

    await expect(service.invoke('open_project', { path: '/tmp/project' })).resolves.toEqual({
      command: 'open_project',
      payload: { path: '/tmp/project' },
    });
  });

  it('supports replacing the transport in Angular tests', async () => {
    const invoke = vi.fn().mockResolvedValue(['first', 'second']);
    TestBed.configureTestingModule({
      providers: [{ provide: IPC_TRANSPORT, useValue: { invoke } }],
    });
    const service = TestBed.inject(IpcService);

    await expect(service.invoke<string[]>('list_projects')).resolves.toEqual(['first', 'second']);
    expect(invoke).toHaveBeenCalledWith('list_projects', undefined);
  });
});
