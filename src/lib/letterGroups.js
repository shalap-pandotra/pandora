const collator = new Intl.Collator(undefined, { sensitivity: 'base' });

export function firstLetter(text) {
  const trimmed = (text ?? '').trim();
  if (!trimmed) return '#';
  const c = [...trimmed.normalize('NFD')][0].toUpperCase();
  return /\p{L}/u.test(c) ? c : '#';
}

/**
 * @template T
 * @param {T[]} rows
 * @param {(row: T) => string} keyOf the text the row sorts by
 * @param {number} minGroup
 * @returns {{ label: string, rows: T[] }[]}
 */
export function letterGroups(rows, keyOf = (r) => r.sort || r.name, minGroup = 5) {
  const sorted = rows.slice().sort((a, b) => collator.compare(keyOf(a), keyOf(b)));
  const letters = sorted.map((r) => firstLetter(keyOf(r)));

  /** @type {{ first: string, last: string, length: number }[]} */
  let intervals = [];
  for (const letter of letters) {
    const last = intervals[intervals.length - 1];
    if (last && last.first === letter) last.length++;
    else intervals.push({ first: letter, last: letter, length: 1 });
  }
  const single = intervals.map((i) => ({ ...i }));

  // Merge groups smaller than minGroup into a neighbour, the smaller neighbour first, until none is.
  if (minGroup > 1) {
    const none = { first: '', last: '', length: 100000000 };
    let changed = true;
    while (changed && intervals.length > 1) {
      changed = false;
      for (let idx = 0; idx < intervals.length; idx++) {
        const interval = intervals[idx];
        if (interval.length >= minGroup) continue;
        let prev = none;
        let next = none;
        if (idx === 0) next = intervals[idx + 1];
        else {
          prev = intervals[idx - 1];
          if (idx < intervals.length - 1) next = intervals[idx + 1];
        }
        if (prev.length < next.length) intervals[idx - 1] = { first: prev.first, last: interval.last, length: prev.length + interval.length };
        else intervals[idx + 1] = { first: interval.first, last: next.last, length: next.length + interval.length };
        intervals.splice(idx, 1);
        changed = true;
        break;
      }
    }
  }
  // Only one group left: no point in ranges, so go back to a group per letter.
  if (intervals.length <= 1) intervals = single;

  const groups = [];
  let at = 0;
  for (const { first, last, length } of intervals) {
    groups.push({ label: first === last ? first : `${first} - ${last}`, rows: sorted.slice(at, at + length) });
    at += length;
  }
  return groups;
}
