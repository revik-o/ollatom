export const SUPPORTED_LOCALES = ['en', 'ua', 'ru'] as const;

export type Locale = (typeof SUPPORTED_LOCALES)[number];
