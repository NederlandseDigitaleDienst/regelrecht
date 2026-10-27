/** `**bold**` → <strong>, `` `name` `` → <code>; everything else escaped. For the deck and the explanation page. */
export function emphasize(line) {
  const escaped = String(line).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
  return escaped.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>').replace(/`([^`]+)`/g, '<code>$1</code>');
}
