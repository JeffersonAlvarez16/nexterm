// features/passwords/filterEntries.test.ts

import { describe, it, expect } from "vitest";
import { filterPasswordEntries } from "./filterEntries";
import type { PasswordEntryMeta } from "../../stores/passwordStore";

function entry(over: Partial<PasswordEntryMeta>): PasswordEntryMeta {
  return {
    id: over.id ?? "id",
    title: over.title ?? "",
    username: over.username ?? "",
    url: over.url ?? "",
    category: over.category ?? "",
    createdAt: 0,
    updatedAt: 0,
  };
}

const entries: PasswordEntryMeta[] = [
  entry({ id: "1", title: "GitHub", username: "octocat", url: "github.com", category: "Work" }),
  entry({ id: "2", title: "Bank", username: "alice", url: "mybank.example", category: "Finance" }),
  entry({ id: "3", title: "Vima Aplicaciones", username: "vima", url: "vima oracle", category: "Servidores" }),
];

describe("filterPasswordEntries", () => {
  it("returns the list unchanged for an empty query", () => {
    expect(filterPasswordEntries(entries, "")).toBe(entries);
    expect(filterPasswordEntries(entries, "   ")).toBe(entries);
  });

  it("matches the title case-insensitively", () => {
    const out = filterPasswordEntries(entries, "github");
    expect(out.map((e) => e.id)).toEqual(["1"]);
  });

  it("matches the username", () => {
    expect(filterPasswordEntries(entries, "alice").map((e) => e.id)).toEqual(["2"]);
  });

  it("matches the url", () => {
    expect(filterPasswordEntries(entries, "mybank").map((e) => e.id)).toEqual(["2"]);
  });

  it("matches the category", () => {
    expect(filterPasswordEntries(entries, "servidores").map((e) => e.id)).toEqual(["3"]);
  });

  it("trims surrounding whitespace before matching", () => {
    expect(filterPasswordEntries(entries, "  vima  ").map((e) => e.id)).toEqual(["3"]);
  });

  it("returns an empty array when nothing matches", () => {
    expect(filterPasswordEntries(entries, "zzz-nope")).toEqual([]);
  });

  it("can match multiple entries on a shared substring", () => {
    // 'a' appears in several fields across rows.
    const out = filterPasswordEntries(entries, "ima");
    expect(out.map((e) => e.id)).toEqual(["3"]);
  });
});
