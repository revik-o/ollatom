import {
  AfterViewInit,
  ChangeDetectionStrategy,
  Component,
  ElementRef,
  OnDestroy,
  inject,
} from '@angular/core';
import { NgStyle } from '@angular/common';
import { OsWindowService, WindowResizeDirection } from '../../core/os-window/os-window.service';
import { WindowControlsComponent } from '../window-controls/window-controls.component';

@Component({
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NgStyle, WindowControlsComponent],
  selector: 'app-window-frame',
  styleUrl: './window-frame.component.css',
  templateUrl: './window-frame.component.html',
})
export class WindowFrameComponent implements AfterViewInit, OnDestroy {
  private readonly element = inject(ElementRef<HTMLElement>);
  private readonly osWindowService = inject(OsWindowService);
  private resizeObserver: ResizeObserver | undefined;

  public ngAfterViewInit(): void {
    if (typeof ResizeObserver === 'undefined') {
      return;
    }

    this.resizeObserver = new ResizeObserver(() => void this.publishInteractiveRegions());
    this.resizeObserver.observe(this.element.nativeElement);
    void this.publishInteractiveRegions();
  }

  public ngOnDestroy(): void {
    this.resizeObserver?.disconnect();
  }

  private async publishInteractiveRegions(): Promise<void> {
    const host = this.element.nativeElement as HTMLElement;
    const frame = host.getBoundingClientRect();
    const regions = Array.from(host.querySelectorAll<HTMLElement>('[data-window-interactive]'))
      .map((element) => element.getBoundingClientRect())
      .map((rect) => ({ x: rect.x - frame.x, y: rect.y - frame.y, width: rect.width, height: rect.height }));

    await this.osWindowService. setWindowInteractiveRegions(regions);
  }

  protected async resizeWindow(direction: WindowResizeDirection): Promise<void> {
    await this.osWindowService.resizeWindow(direction);
  }

  protected async dragWindow(): Promise<void> {
    await this.osWindowService.dragWindow();
  }

  protected hasClientControls(): boolean {
    return this.osWindowService.hasClientControls();
  }

  protected controlsOnLeft(): boolean {
    return this.osWindowService.controlsOnLeft();
  }

  protected nativeStandardWindow(): boolean {
    return this.osWindowService.nativeStandardWindow();
  }

  protected titleBarStyle() {
    return this.osWindowService.titleBarStyle();
  }
}
