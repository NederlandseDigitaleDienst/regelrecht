// Show an annotated fragment with a little surrounding context so the quote
// reads as part of a sentence, not a bare clipping.
//
// Rules (from the product owner):
// - Up to 3 context words on each side of the fragment.
// - Left: stop at a sentence start — a capitalised word — and include it.
// - Right: stop at a period (sentence end) and include up to it.
// - The ellipsis (…) only appears when we actually truncated (hit the 3-word
//   cap with more text beyond), and it sits OUTSIDE the italic fragment.
//
// Returns display-ready strings so the template stays trivial and only the
// `quote` is wrapped in <i>:
//   {{ellipsisBefore}}{{before}}<i>{{quote}}</i>{{after}}{{ellipsisAfter}}

const MAX_CONTEXT_WORDS = 3;
const STARTS_CAPITAL = /^\p{Lu}/u;

/**
 * @param {string} text        The full source text.
 * @param {number} startUtf16  UTF-16 index where the fragment starts.
 * @param {number} endUtf16    UTF-16 index where the fragment ends.
 */
export function quoteContext(text: string, startUtf16: number, endUtf16: number) {
  const quote = text.slice(startUtf16, endUtf16);

  // Left context: walk words backwards, stopping at a capitalised word (the
  // sentence's start) or after 3 words.
  const beforeWords = text.slice(0, startUtf16).match(/\S+/g) || [];
  let beforeTaken = 0;
  let reachedSentenceStart = false;
  for (let i = beforeWords.length - 1; i >= 0 && beforeTaken < MAX_CONTEXT_WORDS; i--) {
    beforeTaken++;
    // i stays within the loop bounds.
    if (STARTS_CAPITAL.test(beforeWords[i]!)) {
      reachedSentenceStart = true;
      break;
    }
  }
  const before = beforeWords.slice(beforeWords.length - beforeTaken).join(' ');
  // Ellipsis only when we stopped early (the 3-word cap) and more text remains.
  const truncatedBefore = !reachedSentenceStart && beforeTaken < beforeWords.length;

  // Right context: walk words forwards, stopping at the first period (sentence
  // end, kept) or after 3 words.
  const afterWords = text.slice(endUtf16).match(/\S+/g) || [];
  let afterTaken = 0;
  let reachedSentenceEnd = false;
  const afterParts = [];
  for (let i = 0; i < afterWords.length && afterTaken < MAX_CONTEXT_WORDS; i++) {
    afterTaken++;
    const word = afterWords[i]!; // i < afterWords.length
    const dot = word.indexOf('.');
    if (dot >= 0) {
      afterParts.push(word.slice(0, dot + 1));
      reachedSentenceEnd = true;
      break;
    }
    afterParts.push(word);
  }
  const after = afterParts.join(' ');
  const truncatedAfter = !reachedSentenceEnd && afterTaken < afterWords.length;

  return {
    quote,
    before: before ? `${before} ` : '',
    after: after ? ` ${after}` : '',
    ellipsisBefore: truncatedBefore ? '… ' : '',
    ellipsisAfter: truncatedAfter ? ' …' : '',
  };
}
