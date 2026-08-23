import { TestBed } from '@angular/core/testing';
import { ApplicationConfigService } from '../application-config/application-config.service';
import { LanguageService } from './language.service';

describe('LanguageService', () => {
  let read: ReturnType<typeof vi.fn>;
  let add: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    read = vi.fn().mockRejectedValue(new Error('configuration value does not exist'));
    add = vi.fn().mockResolvedValue('success');
    TestBed.configureTestingModule({
      providers: [{ provide: ApplicationConfigService, useValue: { read, add } }],
    });
  });

  it('switches catalogs and updates the document language', async () => {
    const service = TestBed.inject(LanguageService);

    await service.setLocale('ua');

    expect(service.text.common.welcome).toBe('Вітаємо');
    expect(document.documentElement.lang).toBe('ua');
  });

  it('persists the selected locale', async () => {
    const service = TestBed.inject(LanguageService);

    await service.setLocale('ru');

    expect(add).toHaveBeenCalledWith('app.language', 'ru');
  });

  it('supports typed messages with parameters', async () => {
    const service = TestBed.inject(LanguageService);

    await service.setLocale('en');

    expect(service.text.files.selected({ count: 2 })).toBe('2 files selected');
  });

  it('loads the configured locale', async () => {
    read.mockResolvedValue('ru');
    const service = TestBed.inject(LanguageService);

    await service.initialized;

    expect(service.text.common.welcome).toBe('Добро пожаловать');
    expect(document.documentElement.lang).toBe('ru');
  });

  it('does not replace a user selection with a late configuration read', async () => {
    let resolveRead: (value: string) => void = () => undefined;
    read.mockImplementation(
      () =>
        new Promise<string>((resolve) => {
          resolveRead = resolve;
        }),
    );
    const service = TestBed.inject(LanguageService);

    await service.setLocale('ua');
    resolveRead('ru');
    await service.initialized;

    expect(service.text.common.welcome).toBe('Вітаємо');
    expect(document.documentElement.lang).toBe('ua');
  });
});
