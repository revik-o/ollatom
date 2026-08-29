import { Messages } from "../../messages";
import { pluralizeSlavic } from "./pluralize";

export const ua = {
  common: {
    close: 'Закрити',
    welcome: 'Вітаємо',
  },
  files: {
    selected: ({ count = 0 }: { count?: number } = {}) =>
      `Вибрано ${count} ${pluralizeSlavic(count, { one: 'файл', few: 'файли', many: 'файлів' })}`,
  },
  language: {
    english: 'Англійська',
    russian: 'Російська',
    ukrainian: 'Українська',
  },
} satisfies Messages;
