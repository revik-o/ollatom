export type SlavicPluralization = {
  one: string;
  few: string;
  many: string;
};

export function pluralizeSlavic(count: number, words: SlavicPluralization): string {
  const lastTwoDigits = count % 100;
  const lastDigit = count % 10;

  if (lastTwoDigits >= 11 && lastTwoDigits <= 14) {
    return words.many;
  }

  if (lastDigit === 1) {
    return words.one;
  }

  if (lastDigit >= 2 && lastDigit <= 4) {
    return words.few;
  }

  return words.many;
}
