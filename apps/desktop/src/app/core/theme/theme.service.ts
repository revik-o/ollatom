import { computed, inject, Injectable, signal } from '@angular/core';
import { type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow, type Theme } from '@tauri-apps/api/window';
import { ApplicationConfigService } from '../application-config/application-config.service';
import { OsWindowService } from '../os-window/os-window.service';

export type ThemePreference = 'system' | 'black' | 'white';
export type ResolvedTheme = 'black' | 'white';

const APPLICATION_THEME_CONFIGURATION_KEY = 'app.theme';
const THEME_CACHE_KEY = 'ollatom.theme.preference';

function isThemePreference(value: unknown): value is ThemePreference {
  return value === 'system' || value === 'black' || value === 'white';
}

function resolveSystemTheme(theme: Theme | null | undefined): ResolvedTheme {
  return theme === 'dark' ? 'black' : 'white';
}

function browserSystemTheme(): ResolvedTheme {
  return typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches
    ? 'black'
    : 'white';
}

@Injectable({ providedIn: 'root' })
export class ThemeService {
  private readonly osWindowService = inject(OsWindowService);
  private readonly applicationConfig = inject(ApplicationConfigService);
  private readonly preferenceState = signal<ThemePreference>('system');
  private readonly systemThemeState = signal<ResolvedTheme>(browserSystemTheme());
  private changeVersion = 0;
  private unlistenThemeChange: UnlistenFn | undefined;
  private mediaQueryUnlisten: (() => void) | undefined;

  public readonly preference = this.preferenceState.asReadonly();
  public readonly resolvedTheme = computed<ResolvedTheme>(() => {
    const preference = this.preferenceState();
    return preference === 'system' ? this.systemThemeState() : preference;
  });
  public readonly initialized: Promise<void>;

  public constructor() {
    this.apply(this.resolvedTheme());
    this.initialized = this.initialize();
  }

  public async setPreference(preference: ThemePreference): Promise<void> {
    const previousPreference = this.preferenceState();
    const changeVersion = ++this.changeVersion;
    this.preferenceState.set(preference);
    await this.apply(this.resolvedTheme());

    try {
      await this.applicationConfig.addProperty(APPLICATION_THEME_CONFIGURATION_KEY, preference);
      this.writeFirstPaintCache(preference);
    } catch (error) {
      if (changeVersion === this.changeVersion) {
        this.preferenceState.set(previousPreference);
        await this.apply(this.resolvedTheme());
      }

      throw error;
    }
  }

  private async initialize(): Promise<void> {
    await this.initializeNativeThemeListener();

    const changeVersion = this.changeVersion;
    try {
      const configuredPreference = await this.applicationConfig.readProperty(
        APPLICATION_THEME_CONFIGURATION_KEY,
      );

      if (changeVersion === this.changeVersion && isThemePreference(configuredPreference)) {
        this.preferenceState.set(configuredPreference);
      }
    } catch {
      // A missing preference defaults to following the system theme.
    }

    await this.apply(this.resolvedTheme());
    this.writeFirstPaintCache(this.preferenceState());
  }

  private async initializeNativeThemeListener(): Promise<void> {
    try {
      const currentWindow = getCurrentWindow();
      const nativeTheme = await currentWindow.theme();
      this.systemThemeState.set(resolveSystemTheme(nativeTheme));
      this.unlistenThemeChange = await currentWindow.onThemeChanged(({ payload }) => {
        this.systemThemeState.set(resolveSystemTheme(payload));

        if (this.preferenceState() === 'system') {
          void this.apply(this.resolvedTheme());
        }
      });
    } catch {
      this.systemThemeState.set(browserSystemTheme());

      if (typeof window !== 'undefined') {
        const query = window.matchMedia('(prefers-color-scheme: dark)');
        const onChange = () => {
          this.systemThemeState.set(browserSystemTheme());
          if (this.preferenceState() === 'system') {
            void this.apply(this.resolvedTheme());
          }
        };
        query.addEventListener('change', onChange);
        this.mediaQueryUnlisten = () => query.removeEventListener('change', onChange);
      }
    }
  }

  private async apply(theme: ResolvedTheme): Promise<void> {
    if (typeof document !== 'undefined') {
      document.documentElement.dataset['theme'] = theme;
      document.documentElement.style.colorScheme = theme === 'black' ? 'dark' : 'light';
    }

    try {
      const currentWindow = getCurrentWindow();
      await currentWindow.setTheme(
        this.preferenceState() === 'system' ? null : theme === 'black' ? 'dark' : 'light',
      );
    } catch {
      // Browser tests and unsupported desktop environments use CSS only.
    }

    await this.osWindowService.setWindowAppearance(theme);
  }

  private writeFirstPaintCache(preference: ThemePreference): void {
    try {
      if (typeof localStorage === 'undefined') return;
      if (preference === 'system') localStorage.removeItem(THEME_CACHE_KEY);
      else localStorage.setItem(THEME_CACHE_KEY, preference);
    } catch {
      // Cache failure must not turn a recoverable preference failure into startup failure.
    }
  }
}
