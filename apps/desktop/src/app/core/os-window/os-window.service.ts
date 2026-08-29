import { computed, inject, Injectable, signal } from "@angular/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { BackdropKind, IpcService, StartupSnapshot, WindowChromeMetrics, WindowChromeMode, WindowControlsName, WindowInteractiveRegion } from "../ipc/ipc.service";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export type WindowResizeDirection =
  'North' | 'NorthEast' | 'East' | 'SouthEast' | 'South' | 'SouthWest' | 'West' | 'NorthWest';

const FALLBACK_SNAPSHOT: StartupSnapshot = {
  backdrop: 'opaque',
  chrome: {
    mode: 'native_standard',
    controls: [],
    titleBarHeight: 0,
    controlsSide: 'right',
    controlsInsetStart: 0,
    controlsInsetEnd: 0,
    scaleFactor: 1,
  },
};

const BACKDROPS = new Set<BackdropKind>(['acrylic', 'vibrancy', 'wayland_blur', 'opaque']);
const CONTROL_NAMES = new Set(['minimize', 'maximize', 'close']);
const CHROME_MODES = new Set<WindowChromeMode>([
  'native_overlay',
  'client_drawn',
  'native_standard',
]);

const finiteNonNegative = (metric: unknown): metric is number =>
  typeof metric === 'number' && Number.isFinite(metric) && metric >= 0;

function isValidChrome(chrome?: Partial<WindowChromeMetrics>): boolean {
  if (!chrome) {
    return false;
  }

  const isModeValid = CHROME_MODES.has(chrome.mode as WindowChromeMode);
  const areControlsValid = Array.isArray(chrome.controls) &&
                            chrome.controls.every(control => CONTROL_NAMES.has(control));
  const isSideValid = chrome.controlsSide === 'left' || chrome.controlsSide === 'right';
  const isGeometryValid = finiteNonNegative(chrome.titleBarHeight) &&
                            finiteNonNegative(chrome.controlsInsetStart) &&
                            finiteNonNegative(chrome.controlsInsetEnd);
  const isScaleValid = typeof chrome.scaleFactor === 'number' &&
                         Number.isFinite(chrome.scaleFactor) &&
                         chrome.scaleFactor > 0;

  return isModeValid &&
    areControlsValid &&
    isSideValid &&
    isGeometryValid &&
    isScaleValid;
}

function validateStartupSnapshot(value: unknown): StartupSnapshot {
  if (!value || typeof value !== 'object') {
    return FALLBACK_SNAPSHOT;
  }

  const snapshot = value as Partial<StartupSnapshot>;

  const isBackdropValid = BACKDROPS.has(snapshot.backdrop as BackdropKind);

  if (!isBackdropValid || !isValidChrome(snapshot.chrome as Partial<WindowChromeMetrics> | undefined)) {
    return FALLBACK_SNAPSHOT;
  }

  return value as StartupSnapshot;
}

@Injectable({ providedIn: 'root' })
export class OsWindowService {
  private readonly ipc = inject(IpcService)
  private readonly snapshotState = signal<StartupSnapshot>(FALLBACK_SNAPSHOT);
  private readonly snapshot = this.snapshotState.asReadonly();
  public readonly initialized: Promise<void>;
  private readonly chrome = computed(() => this.snapshot().chrome,);
  private unlistenAppearanceChange: UnlistenFn | undefined;

  public readonly hasClientControls = computed(() => this.chrome().mode === 'client_drawn');
  public readonly controlsOnLeft = computed(() => this.chrome().controlsSide === 'left');
  public readonly titleBarStyle = computed(() => ({
    '--window-titlebar-height': `${this.chrome().titleBarHeight}px`,
    '--window-controls-inset-start': `${this.chrome().controlsInsetStart}px`,
    '--window-controls-inset-end': `${this.chrome().controlsInsetEnd}px`,
  }));

  public constructor() {
    this.initialized = this.initialize();
  }

  public async minimizeWindow(): Promise<void> {
    await getCurrentWindow().minimize();
  }

  public async toggleMaximizeWindow(): Promise<void> {
    await getCurrentWindow().toggleMaximize();
  }

  public async dragWindow(): Promise<void> {
    if (this.chrome().mode === 'native_standard') {
      return;
    }

    await getCurrentWindow().startDragging();
  }

  public async resizeWindow(direction: WindowResizeDirection): Promise<void> {
    if (!this.hasClientControls()) {
      return;
    }

    await getCurrentWindow().startResizeDragging(direction);
  }

  public async closeWindow(): Promise<void> {
    await getCurrentWindow().close();
  }

  public async setWindowAppearance(theme: 'black' | 'white'): Promise<BackdropKind> {
    try {
      const backdrop = await this.ipc.setWindowAppearance(theme);

      this.snapshotState.update((snapshot) => ({ ...snapshot, backdrop }));

      return backdrop;
    } catch {
      return this.snapshot().backdrop;
    }
  }

  public async setWindowInteractiveRegions(regions: ReadonlyArray<WindowInteractiveRegion>): Promise<void> {
    await this.ipc.setWindowInteractiveRegions(regions);
  }

  public nativeStandardWindow(): boolean {
    return this.chrome().mode === 'native_standard';
  }

  public controlOrder(control: WindowControlsName): number {
    const index = this.chrome().controls.indexOf(control);
    return index < 0 ? 99 : index;
  }

  private applySnapshot(value: StartupSnapshot): void {
    const snapshot = validateStartupSnapshot(value);

    this.snapshotState.set(snapshot);
    document.documentElement.dataset['backdrop'] = snapshot.backdrop;
  }

  private async initialize(): Promise<void> {
    try {
      if (this.ipc.supportsIPCEvents()) {
        this.unlistenAppearanceChange = await this.ipc.listenWindowAppearanceChanged(({ payload }) => this.applySnapshot(payload));
      }

      const snapshot = await this.ipc.waitForBackgroundReady();

      this.applySnapshot(snapshot);
    } catch (error) {
      if (this.ipc.isIPCRuntime()) {
        throw error;
      }
    }
  }
}
