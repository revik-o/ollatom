import { inject, Injectable, signal } from '@angular/core';
import { ApplicationConfigService } from '../application-config/application-config.service';
import { type Status } from '../ipc/ipc.service';
import { SUPPORTED_LOCALES, type Locale } from './locale.definition';
import { Messages } from './messages';
import { en } from './locales/en';
import { ru } from './locales/ru';
import { ua } from './locales/ua';

const CATALOGS = { en, ru, ua } as const satisfies Record<Locale, Messages>;
const applicationLanguageConfigurationKey = 'app.language';

function isLocale(value: unknown): value is Locale {
  return SUPPORTED_LOCALES.some((locale) => locale === value);
}

function normalizeLocale(language: string): Locale | undefined {
  const locale = language.toLowerCase().split('-')[0];
  return isLocale(locale) ? locale : undefined;
}

@Injectable({ providedIn: 'root' })
export class LanguageService {
  private readonly applicationConfig = inject(ApplicationConfigService);
  private readonly locale = signal<Locale>(this.detectBrowserLocale());
  private localeChangeVersion = 0;
  public readonly initialized: Promise<void>;

  public get text(): Messages {
    return CATALOGS[this.locale()];
  }

  public constructor() {
    this.applyLocale(this.locale());
    this.initialized = this.initializeLocale();
  }

  public setLocale(locale: Locale): Promise<Status> {
    this.localeChangeVersion += 1;
    this.locale.set(locale);
    this.applyLocale(locale);
    return this.applicationConfig.addProperty(applicationLanguageConfigurationKey, locale);
  }

  private applyLocale(locale: Locale): void {
    if (typeof document !== 'undefined') {
      document.documentElement.lang = locale;
    }
  }

  private async initializeLocale(): Promise<void> {
    const localeChangeVersion = this.localeChangeVersion;
    let configuredLocale: unknown;

    try {
      configuredLocale = await this.applicationConfig.readProperty(applicationLanguageConfigurationKey);
    } catch {
      return;
    }

    if (localeChangeVersion !== this.localeChangeVersion || !isLocale(configuredLocale)) {
      return;
    }

    this.locale.set(configuredLocale);
    this.applyLocale(configuredLocale);
  }

  private detectBrowserLocale(): Locale {
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
