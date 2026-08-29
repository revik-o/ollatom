import { Component, inject } from '@angular/core';
import { OsWindowService } from '../../core/os-window/os-window.service';
import { WindowControlsName } from '../../core/ipc/ipc.service';

@Component({
  imports: [],
  selector: 'app-window-controls',
  styleUrl: './window-controls.css',
  templateUrl: './window-controls.html',
})
export class WindowControlsComponent {
  private readonly osWindowService = inject(OsWindowService);

  protected async minimizeWindow(): Promise<void> {
    await this.osWindowService.minimizeWindow();
  }

  protected async toggleMaximizeWindow(): Promise<void> {
    await this.osWindowService.toggleMaximizeWindow();
  }

  protected async closeWindow(): Promise<void> {
    await this.osWindowService.closeWindow();
  }

  protected controlOrder(control: WindowControlsName): number {
    return this.osWindowService.controlOrder(control);
  }

  protected controlsOnLeft(): boolean {
    return this.osWindowService.controlsOnLeft();
  }
}
