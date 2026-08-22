function pluralizeFiles({ count = 0 }: { count?: number } = {}): string {
  return `${count} ${count === 1 ? 'file' : 'files'} selected`;
}

export const en = {
  common: {
    close: 'Close',
    welcome: 'Welcome',
  },
  files: {
    selected: pluralizeFiles,
  },
  language: {
    english: 'English',
    russian: 'Russian',
    ukrainian: 'Ukrainian',
  },
} as const;
