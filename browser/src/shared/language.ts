/**
 * Language tags, as page translation reads them. Main compares a page's
 * language with the person's and names it in the menu; the shell names it on
 * the address-bar chip. One implementation, so both say the same thing.
 */

/** A tag's primary language ("ja" from "ja-JP"), lower case; '' for nothing. */
export const primaryLanguage = (tag: string | undefined): string =>
  String(tag || '')
    .split('-')[0]
    .toLowerCase();

/** A tag's language named in `locale` ("Japanese" for "ja-JP" in "en"); the tag itself when Intl does not know it. */
export function languageName(tag: string, locale: string): string {
  try {
    return new Intl.DisplayNames([locale], { type: 'language' }).of(primaryLanguage(tag)) || tag;
  } catch {
    return tag;
  }
}
