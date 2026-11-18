// One readable line for a tool call in the policy assistant's feed.
//
// The arguments come straight from the model, so any shape can arrive. The
// naive `${key}=${value}` rendered a list of options as
// "[object Object],[object Object]", which is what ended up on screen.

const MAX_VALUE_LENGTH = 120;

function shorten(text: string): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  return flat.length > MAX_VALUE_LENGTH ? `${flat.slice(0, MAX_VALUE_LENGTH - 1)}…` : flat;
}

// The field a person would recognise an object by: the options of a question
// carry a `label`, most other objects a name or a title.
function nameOf(value: Record<string, unknown>): string | null {
  for (const key of ['label', 'naam', 'titel', 'name', 'title', 'id']) {
    const field = value[key];
    if (typeof field === 'string' && field.trim()) return field.trim();
  }
  return null;
}

export function formatToolValue(value: unknown): string {
  if (value === null || value === undefined) return '';
  if (typeof value === 'string') return shorten(value);
  if (typeof value !== 'object') return String(value);
  if (Array.isArray(value)) {
    const parts = value.map((item: unknown) =>
      item && typeof item === 'object' && !Array.isArray(item)
        ? nameOf(item as Record<string, unknown>)
        : formatToolValue(item),
    );
    // A list whose items have no recognisable name says more as a count than
    // as a row of JSON.
    if (parts.some((p) => !p)) return `${value.length} items`;
    return shorten(parts.join(' / '));
  }
  return nameOf(value as Record<string, unknown>) ?? shorten(JSON.stringify(value));
}

export function formatToolCall(name: string, input: unknown): string {
  const args =
    input && typeof input === 'object'
      ? Object.entries(input)
          .map(([key, value]) => [key, formatToolValue(value)])
          .filter(([, text]) => text !== '')
          .map(([key, text]) => `${key}: ${text}`)
      : [];
  return args.length ? `${name} · ${args.join(' · ')}` : name;
}
