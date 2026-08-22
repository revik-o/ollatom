import { Messages } from '../messages';

function pluralizeFiles(count: number): string {
  const lastTwoDigits = count % 100;
  const lastDigit = count % 10;

  if (lastTwoDigits >= 11 && lastTwoDigits <= 14) {
    return 'файлов';
  }

  if (lastDigit === 1) {
    return 'файл';
  }

  if (lastDigit >= 2 && lastDigit <= 4) {
    return 'файла';
  }

  return 'файлов';
}

export const ru = {
  common: {
    close: 'Закрыть',
    welcome: 'Добро пожаловать',
  },
  files: {
    selected: ({ count = 0 }: { count?: number } = {}) =>
      `Выбрано ${count} ${pluralizeFiles(count)}`,
  },
  language: {
    english: 'Английский',
    russian: 'Русский',
    ukrainian: 'Украинский',
  },
} satisfies Messages;
