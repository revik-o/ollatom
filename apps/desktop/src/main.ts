import { bootstrapApplication } from '@angular/platform-browser';
import { appConfig } from './app/app.config';
import { App } from './app/app';

const SPLASH_EXIT_ANIMATION = 'ollatom-splash-exit';
const appRoot = () => document.querySelector('app-root');
const splashElement = () => document.getElementById('app-splash');

const waitRenderReadiness = () => new Promise<void>(
  (resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
);

function finishSplash(): void {
  const root = appRoot();

  splashElement()?.remove();
  document.getElementById('app-splash-critical-styles')?.remove();
  root?.removeAttribute('inert');
  root?.removeAttribute('aria-hidden');
  document.documentElement.dataset['uiExposed'] = 'true';
  document.body.removeAttribute('aria-busy');
}

async function dismissSplash(): Promise<void> {
  const splash = splashElement();
  const root = appRoot();

  if (!splash || !root) {
    finishSplash();
    return;
  }

  document.documentElement.dataset['uiPrepared'] = 'true';

  await waitRenderReadiness();

  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    finishSplash();
    return;
  }

  await new Promise<void>((resolve) => {
    const fallback = window.setTimeout(resolve, 500);

    splash.addEventListener(
      'animationend',
      (event) => {
        if (event.animationName === SPLASH_EXIT_ANIMATION) {
          window.clearTimeout(fallback);
          resolve();
        }
      },
      { once: true },
    );

    splash.classList.add('app-splash--exiting');
  });

  finishSplash();
}

function showStartupFailure(error: unknown): void {
  console.error('Ollatom startup failed', error);
  const splash = splashElement();

  if (!splash) {
    return;
  }

  splash.classList.remove('app-splash--exiting');
  splash.dataset['state'] = 'error';

  const skeleton = splash.querySelector('.app-splash__skeleton');
  skeleton?.remove();

  const errorState = document.createElement('div');

  errorState.className = 'app-splash__error';
  errorState.innerHTML = '<p>Ollatom could not start.</p><button type="button">Retry</button>';
  errorState.querySelector('button')?.addEventListener('click', () => window.location.reload());

  splash.append(errorState);
}

bootstrapApplication(App, appConfig)
  .then(async () => {
    await document.fonts?.ready;
    await waitRenderReadiness()
    await dismissSplash();
  })
  .catch(showStartupFailure);
