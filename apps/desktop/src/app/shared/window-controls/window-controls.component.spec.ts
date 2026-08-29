import { ComponentFixture, TestBed } from '@angular/core/testing';
import { WindowControlsComponent } from './window-controls.component';

describe('WindowControls', () => {
  let component: WindowControlsComponent;
  let fixture: ComponentFixture<WindowControlsComponent>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [WindowControlsComponent],
    }).compileComponents();

    fixture = TestBed.createComponent(WindowControlsComponent);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
