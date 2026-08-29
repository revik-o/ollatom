import { Component } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { WindowFrameComponent } from './shared/window-frame/window-frame.component';

@Component({
  imports: [RouterOutlet, WindowFrameComponent],
  selector: 'app-root',
  styleUrl: './app.css',
  templateUrl: './app.html',
})
export class App { }
