import { en } from './locales/en';

type WidenCatalog<T> = T extends (...args: infer Args) => string
  ? (...args: Args) => string
  : T extends string
    ? string
    : { readonly [Key in keyof T]: WidenCatalog<T[Key]> };

export type Messages = WidenCatalog<typeof en>;

type StringLeafKeys<T, Prefix extends string = ''> = {
  [Key in keyof T & string]: T[Key] extends string
    ? `${Prefix}${Key}`
    : T[Key] extends (...args: infer _Args) => string
      ? never
      : StringLeafKeys<T[Key], `${Prefix}${Key}.`>;
}[keyof T & string];

export type I18nTextKey = StringLeafKeys<Messages>;

type MessageLeafKeys<T, Prefix extends string = ''> = {
  [Key in keyof T & string]: T[Key] extends string
    ? `${Prefix}${Key}`
    : T[Key] extends (...args: infer _Args) => string
      ? `${Prefix}${Key}`
      : MessageLeafKeys<T[Key], `${Prefix}${Key}.`>;
}[keyof T & string];

type MessageAtPath<T, Path extends string> = Path extends `${infer Head}.${infer Tail}`
  ? Head extends keyof T
    ? MessageAtPath<T[Head], Tail>
    : never
  : Path extends keyof T
    ? T[Path]
    : never;

export type I18nMessageKey = MessageLeafKeys<Messages>;

export type I18nMessageParams<Key extends I18nMessageKey> = Key extends I18nMessageKey
  ? MessageAtPath<Messages, Key> extends (...args: infer Args) => string
    ? Args extends []
      ? never
      : Args[0]
    : never
  : never;
