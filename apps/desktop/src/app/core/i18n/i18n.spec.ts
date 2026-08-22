import { Component, signal } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import { LanguageService } from './language.service';
import { I18nMessageKey, I18nMessageParams, I18nTextKey } from './messages';
import { I18nTxt } from './i18n';

@Component({
  imports: [I18nTxt],
  template: '<i18n-txt message="common.welcome" />',
})
class StaticTestHost {}

@Component({
  imports: [I18nTxt],
  template: '<i18n-txt message="files.selected" [params]="{ count: selectedCount() }" />',
})
class ParameterizedTestHost {
  readonly selectedCount = signal(2);
}

describe('I18nTxt', () => {
  beforeEach(async () => {
    localStorage.clear();
    await TestBed.configureTestingModule({
      imports: [StaticTestHost, ParameterizedTestHost],
    }).compileComponents();
  });

  it('renders a statically typed translation key', () => {
    const fixture = TestBed.createComponent(StaticTestHost);

    fixture.detectChanges();

    expect(fixture.nativeElement.textContent).toContain('Welcome');
  });

  it('renders a parameterized translation', () => {
    const fixture = TestBed.createComponent(ParameterizedTestHost);

    fixture.detectChanges();

    expect(fixture.nativeElement.textContent).toContain('2 files selected');
  });

  it('updates when translation parameters change', () => {
    const fixture = TestBed.createComponent(ParameterizedTestHost);

    fixture.detectChanges();
    fixture.componentInstance.selectedCount.set(1);
    fixture.detectChanges();

    expect(fixture.nativeElement.textContent).toContain('1 file selected');
  });

  it('updates when the locale changes', () => {
    const fixture = TestBed.createComponent(StaticTestHost);
    const language = TestBed.inject(LanguageService);

    fixture.detectChanges();
    language.setLocale('ua');
    fixture.detectChanges();

    expect(fixture.nativeElement.textContent).toContain('Вітаємо');
  });

  it('renders parameterized messages from the current locale', () => {
    const fixture = TestBed.createComponent(ParameterizedTestHost);
    const language = TestBed.inject(LanguageService);

    fixture.detectChanges();
    language.setLocale('ua');
    fixture.detectChanges();

    expect(fixture.nativeElement.textContent).toContain('Вибрано 2 файли');
  });

  it('derives message keys and their parameter types from the catalog', () => {
    // @ts-expect-error Unknown translation keys must fail during compilation.
    const misspelledKey: I18nMessageKey = 'common.welcom';
    // @ts-expect-error Parameterized messages are not static text keys.
    const parameterizedKey: I18nTextKey = 'files.selected';
    const validParams: I18nMessageParams<'files.selected'> = { count: 2 };
    // @ts-expect-error The count parameter must be numeric.
    const invalidParams: I18nMessageParams<'files.selected'> = { count: '2' };
    // @ts-expect-error Static messages do not accept parameters.
    const staticParams: I18nMessageParams<'common.welcome'> = {};

    expect([misspelledKey, parameterizedKey, validParams, invalidParams, staticParams]).toEqual([
      'common.welcom',
      'files.selected',
      { count: 2 },
      { count: '2' },
      {},
    ]);
  });
});
