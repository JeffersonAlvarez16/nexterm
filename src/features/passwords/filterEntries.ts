// features/passwords/filterEntries.ts — pure search filter for password entries
//
// Extracted as a pure function so the matching rules are unit-testable in
// isolation, independent of the list component and its secret-handling state.

import type { PasswordEntryMeta } from "../../stores/passwordStore";

/**
 * Filter password entries by a free-text query, matched case-insensitively
 * against the title, username, url, and category. An empty or whitespace-only
 * query returns the list unchanged (same array reference).
 *
 * Note: only metadata fields are searched — the password is never decrypted or
 * available here, so it can never be matched against.
 */
export function filterPasswordEntries(
  entries: PasswordEntryMeta[],
  query: string,
): PasswordEntryMeta[] {
  const q = query.trim().toLowerCase();
  if (!q) return entries;
  return entries.filter((entry) =>
    [entry.title, entry.username, entry.url, entry.category].some((field) =>
      field?.toLowerCase().includes(q),
    ),
  );
}
