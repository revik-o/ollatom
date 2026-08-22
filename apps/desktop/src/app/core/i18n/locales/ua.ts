import { Messages } from '../messages';

function pluralizeFiles(count: number): string {
  const lastTwoDigits = count % 100;
  const lastDigit = count % 10;

  if (lastTwoDigits >= 11 && lastTwoDigits <= 14) {
    return 'файлів';
  }

  if (lastDigit === 1) {
    return 'файл';
  }

  if (lastDigit >= 2 && lastDigit <= 4) {
    return 'файли';
  }

  return 'файлів';
}

export const ua = {
  common: {
    close: 'Закрити',
    welcome: 'Вітаємо',
  },
  files: {
    selected: ({ count = 0 }: { count?: number } = {}) =>
      `Вибрано ${count} ${pluralizeFiles(count)}`,
  },
  language: {
    english: 'Англійська',
    russian: 'Російська',
    ukrainian: 'Українська',
  },
} satisfies Messages;
