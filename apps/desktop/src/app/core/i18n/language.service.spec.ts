import { TestBed } from '@angular/core/testing';
import { LanguageService } from './language.service';

describe('LanguageService', () => {
  beforeEach(() => {
    localStorage.clear();
    TestBed.configureTestingModule({});
  });

  it('switches catalogs and updates the document language', () => {
    const service = TestBed.inject(LanguageService);

    service.setLocale('ua');

    expect(service.text.common.welcome).toBe('Вітаємо');
    expect(document.documentElement.lang).toBe('ua');
  });

  it('persists the selected locale', () => {
    const service = TestBed.inject(LanguageService);

    service.setLocale('ru');

    expect(localStorage.getItem('ollatom.locale')).toBe('ru');
  });

  it('supports typed messages with parameters', () => {
    const service = TestBed.inject(LanguageService);

    service.setLocale('en');

    expect(service.text.files.selected({ count: 2 })).toBe('2 files selected');
  });
});
