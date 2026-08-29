import { Injectable } from '@angular/core';
import { invoke, transformCallback } from '@tauri-apps/api/core';
import { EventCallback, listen, TauriEvent, UnlistenFn } from '@tauri-apps/api/event';

export type BackdropKind = 'acrylic' | 'vibrancy' | 'wayland_blur' | 'opaque';
export type ApplicationConfigValue = string | number;
export type Status = 'success';
export type WindowChromeMode = 'native_overlay' | 'client_drawn' | 'native_standard';
export type WindowControlsName = 'minimize' | 'maximize' | 'close';
export type WindowControlsSide = 'left' | 'right';
export type WindowInteractiveRegion = { x: number; y: number; width: number; height: number };

export interface WindowChromeMetrics {
  mode: WindowChromeMode;
  controls: Array<WindowControlsName>;
  titleBarHeight: number;
  controlsSide: WindowControlsSide;
  controlsInsetStart: number;
  controlsInsetEnd: number;
  scaleFactor: number;
}

export interface StartupSnapshot {
  backdrop: BackdropKind;
  chrome: WindowChromeMetrics;
}

@Injectable({ providedIn: 'root' })
export class IpcService {

  public isIPCRuntime(): boolean {
    return '__TAURI_INTERNALS__' in window;
  }

  public supportsIPCEvents(): boolean {
    return typeof transformCallback === 'function';
  }

  public getApplicationConfigValueByKey(key: string): Promise<ApplicationConfigValue> {
    return invoke<ApplicationConfigValue>('get_application_config_value_by_key', { key });
  }

  public setApplicationConfigValue(key: string, value: ApplicationConfigValue): Promise<Status> {
    return invoke<Status>('set_application_config_value', { key, value });
  }

  public setWindowInteractiveRegions(regions: ReadonlyArray<WindowInteractiveRegion>): Promise<void> {
    return invoke<void>('set_window_interactive_regions', { regions });
  }

  public setWindowAppearance(theme: 'black' | 'white'): Promise<BackdropKind> {
    return invoke<BackdropKind>('set_window_appearance', { theme });
  }

  public windowAppearanceChanged(): Promise<StartupSnapshot> {
    return invoke<StartupSnapshot>('window_appearance_changed');
  }

  public waitForBackgroundReady(): Promise<StartupSnapshot> {
    return invoke<StartupSnapshot>('wait_for_background_ready');
  }

  public listenWindowAppearanceChanged(callback: EventCallback<StartupSnapshot>): Promise<UnlistenFn> {
    return listen<StartupSnapshot>(TauriEvent.WINDOW_THEME_CHANGED, callback);
  }
}
