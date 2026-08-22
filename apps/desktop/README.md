# ollatom desktop

## Usage

```bash
npm run start
npm run build
npm run test
```

## Project organization

This application uses **feature-first architecture** (also known as
**folders-by-feature**): organize code by feature and ownership, not by artifact
type such as components or services.

Follow Angular's
[Organize your project by feature areas](https://angular.dev/style-guide#organize-your-project-by-feature-areas)
guidance.

Application-wide infrastructure belongs under `src/app/core`. For example,
`core/ipc` owns the low-level Tauri IPC bridge and `core/i18n` owns localization.
Keep `shared` for reusable, dependency-light UI building blocks rather than
singletons or platform integration.

Keep feature-specific IPC operations with their owning feature and have them
call `IpcService`. This keeps command names and native DTOs out of components
without growing one application-wide service into a collection of unrelated
feature APIs.
