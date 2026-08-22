import { ChangeDetectionStrategy, Component, computed, inject, input } from '@angular/core';
import { LanguageService } from './language.service';
import { I18nMessageKey, I18nMessageParams, Messages } from './messages';

@Component({
  changeDetection: ChangeDetectionStrategy.OnPush,
  selector: 'i18n-txt',
  template: '{{ text() }}',
})
export class I18nTxt<Message extends I18nMessageKey = I18nMessageKey> {
  public readonly message = input.required<Message>();
  public readonly params = input<I18nMessageParams<NoInfer<Message>>>();
  private readonly language = inject(LanguageService);
  protected readonly text = computed(() =>
    resolveText(this.language.text, this.message(), this.params()),
  );
}

function resolveText(catalog: Messages, message: I18nMessageKey, params: unknown): string {
  let value: unknown = catalog;

  for (const segment of message.split('.')) {
    if (typeof value !== 'object' || value === null || !(segment in value)) {
      throw new Error(`Missing translation: ${message}`);
    }

    value = (value as Record<string, unknown>)[segment];
  }

  if (typeof value === 'string') {
    if (params !== undefined) {
      throw new Error(`Static translation does not accept parameters: ${message}`);
    }

    return value;
  }

  if (typeof value === 'function') {
    return (value as (params?: unknown) => string)(params);
  }

  throw new Error(`Translation is not text: ${message}`);
}
