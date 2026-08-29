import { Messages } from "../../messages";
import { pluralizeSlavic } from "./pluralize";

export const ru = {
  common: {
    close: 'Закрыть',
    welcome: 'Добро пожаловать',
  },
  files: {
    selected: ({ count = 0 }: { count?: number } = {}) =>
      `Выбрано ${count} ${pluralizeSlavic(count, { one: 'файл', few: 'файла', many: 'файлов' })}`,
  },
  language: {
    english: 'Английский',
    russian: 'Русский',
    ukrainian: 'Украинский',
  },
} satisfies Messages;
