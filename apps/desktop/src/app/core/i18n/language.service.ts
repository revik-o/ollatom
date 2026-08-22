import { Injectable, signal } from '@angular/core';
import { SUPPORTED_LOCALES, type Locale } from './locale.definition';
import { Messages } from './messages';
import { en } from './locales/en';
import { ru } from './locales/ru';
import { ua } from './locales/ua';

const CATALOGS = { en, ru, ua } as const satisfies Record<Locale, Messages>;
const localeStorageKey = 'ollatom.locale';

function isLocale(value: string | null): value is Locale {
  return SUPPORTED_LOCALES.some((locale) => locale === value);
}

function normalizeLocale(language: string): Locale | undefined {
  const locale = language.toLowerCase().split('-')[0];
  return isLocale(locale) ? locale : undefined;
}

@Injectable({ providedIn: 'root' })
export class LanguageService {
  private readonly locale = signal<Locale>(this.detectLocale());

  public get text(): Messages {
    return CATALOGS[this.locale()];
  }

  public constructor() {
    this.applyLocale(this.locale(), false);
  }

  public setLocale(locale: Locale): void {
    this.locale.set(locale);
    this.applyLocale(locale, true);
  }

  private applyLocale(locale: Locale, persist: boolean): void {
    if (typeof document !== 'undefined') {
      document.documentElement.lang = locale;
    }

    if (!persist) {
      return;
    }

    localStorage.setItem(localeStorageKey, locale);
  }

  private detectLocale(): Locale {
    const storedLocale = localStorage.getItem(localeStorageKey);

    if (isLocale(storedLocale)) {
      return storedLocale;
    }

    if (typeof navigator !== 'undefined') {
      for (const language of navigator.languages) {
        const locale = normalizeLocale(language);

        if (locale) {
          return locale;
        }
      }
    }

    return 'en';
  }
}
