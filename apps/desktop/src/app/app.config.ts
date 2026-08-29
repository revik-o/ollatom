import {
  ApplicationConfig,
  inject,
  provideAppInitializer,
  provideBrowserGlobalErrorListeners,
} from '@angular/core';
import { provideRouter } from '@angular/router';
import { routes } from './app.routes';
import { LanguageService } from './core/i18n/language.service';
import { ThemeService } from './core/theme/theme.service';
import { OsWindowService } from './core/os-window/os-window.service';

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    provideRouter(routes),
    provideAppInitializer(() =>
      Promise.all([
        inject(LanguageService).initialized,
        inject(ThemeService).initialized,
        inject(OsWindowService).initialized,
      ]),
    ),
  ],
};
