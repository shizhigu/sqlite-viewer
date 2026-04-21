import { describe, expect, it } from "vitest";

import {
  buildDelete,
  buildInsert,
  buildUpdate,
  coerceFromString,
  formatValue,
  quoteIdent,
} from "./sql";

describe("quoteIdent", () => {
  it("wraps a simple identifier", () => {
    expect(quoteIdent("users")).toBe('"users"');
  });

  it("escapes embedded double quotes", () => {
    expect(quoteIdent('weird "name')).toBe('"weird ""name"');
  });

  it("handles empty string", () => {
    expect(quoteIdent("")).toBe('""');
  });

  it("preserves unicode", () => {
    expect(quoteIdent("данные")).toBe('"данные"');
  });
});

describe("buildUpdate", () => {
  it("generates a single-column UPDATE with params in order", () => {
    const { sql, params } = buildUpdate(
      "users",
      { name: "Alice" },
      { id: 42 },
    );
    expect(sql).toBe('UPDATE "users" SET "name" = ?1 WHERE "id" = ?2');
    expect(params).toEqual(["Alice", 42]);
  });

  it("supports multi-column SET and composite PK", () => {
    const { sql, params } = buildUpdate(
      "orders",
      { status: "paid", total: 9.99 },
      { customer: 1, order_no: 7 },
    );
    expect(sql).toBe(
      'UPDATE "orders" SET "status" = ?1, "total" = ?2 WHERE "customer" = ?3 AND "order_no" = ?4',
    );
    expect(params).toEqual(["paid", 9.99, 1, 7]);
  });

  it("quotes identifiers with special characters", () => {
    const { sql } = buildUpdate(
      'tab"le',
      { 'c"ol': 1 },
      { id: 2 },
    );
    expect(sql).toBe(
      'UPDATE "tab""le" SET "c""ol" = ?1 WHERE "id" = ?2',
    );
  });

  it("preserves NULL values in params", () => {
    const { params } = buildUpdate(
      "t",
      { name: null },
      { id: 1 },
    );
    expect(params).toEqual([null, 1]);
  });

  it("throws when updates is empty", () => {
    expect(() => buildUpdate("t", {}, { id: 1 })).toThrow();
  });

  it("throws when pk is empty", () => {
    expect(() => buildUpdate("t", { a: 1 }, {})).toThrow();
  });
});

describe("buildInsert", () => {
  it("generates positional INSERT", () => {
    const { sql, params } = buildInsert("users", { name: "Bob", age: 30 });
    expect(sql).toBe('INSERT INTO "users" ("name", "age") VALUES (?1, ?2)');
    expect(params).toEqual(["Bob", 30]);
  });

  it("falls back to DEFAULT VALUES when no columns provided", () => {
    const { sql, params } = buildInsert("users", {});
    expect(sql).toBe('INSERT INTO "users" DEFAULT VALUES');
    expect(params).toEqual([]);
  });

  it("quotes identifiers", () => {
    const { sql } = buildInsert('weird "name', { 'c"ol': 1 });
    expect(sql).toBe(
      'INSERT INTO "weird ""name" ("c""ol") VALUES (?1)',
    );
  });
});

describe("buildDelete", () => {
  it("generates a single-column DELETE", () => {
    const { sql, params } = buildDelete("t", { id: 3 });
    expect(sql).toBe('DELETE FROM "t" WHERE "id" = ?1');
    expect(params).toEqual([3]);
  });

  it("supports composite PK", () => {
    const { sql, params } = buildDelete("t", { a: 1, b: 2 });
    expect(sql).toBe('DELETE FROM "t" WHERE "a" = ?1 AND "b" = ?2');
    expect(params).toEqual([1, 2]);
  });

  it("throws on empty pk", () => {
    expect(() => buildDelete("t", {})).toThrow();
  });
});

describe("coerceFromString", () => {
  it("coerces integer input", () => {
    expect(coerceFromString("42", "INTEGER", false)).toBe(42);
    expect(coerceFromString("-7", "INTEGER", false)).toBe(-7);
  });

  it("rejects non-integer input for integer columns", () => {
    expect(() => coerceFromString("3.14", "INTEGER", false)).toThrow();
    expect(() => coerceFromString("abc", "INTEGER", false)).toThrow();
  });

  it("coerces real input", () => {
    expect(coerceFromString("3.14", "REAL", false)).toBe(3.14);
    expect(coerceFromString("-1e3", "REAL", false)).toBe(-1000);
  });

  it("rejects NaN for real columns", () => {
    expect(() => coerceFromString("abc", "REAL", false)).toThrow();
  });

  it("passes text through unchanged", () => {
    expect(coerceFromString("hello", "TEXT", false)).toBe("hello");
    expect(coerceFromString("", "TEXT", false)).toBe("");
  });

  it("treats empty string as NULL when allowed", () => {
    expect(coerceFromString("", "TEXT", true)).toBe(null);
    expect(coerceFromString("", null, true)).toBe(null);
  });

  it("keeps empty string when NULL not allowed", () => {
    expect(coerceFromString("", "TEXT", false)).toBe("");
  });

  it("handles null/undefined decl_type as text", () => {
    expect(coerceFromString("anything", null, false)).toBe("anything");
    expect(coerceFromString("anything", undefined, false)).toBe("anything");
  });

  it("treats INT / INTEGER / BIGINT variants as integer", () => {
    expect(coerceFromString("5", "BIGINT", false)).toBe(5);
    expect(coerceFromString("5", "INT", false)).toBe(5);
    expect(coerceFromString("5", "TINYINT", false)).toBe(5);
  });

  it("treats FLOAT / DOUBLE / REAL as real", () => {
    expect(coerceFromString("1.5", "FLOAT", false)).toBe(1.5);
    expect(coerceFromString("1.5", "DOUBLE", false)).toBe(1.5);
    expect(coerceFromString("1.5", "REAL", false)).toBe(1.5);
  });
});

describe("formatValue", () => {
  it("renders NULL", () => {
    expect(formatValue(null)).toBe("NULL");
  });

  it("renders numbers and strings", () => {
    expect(formatValue(42)).toBe("42");
    expect(formatValue("hi")).toBe("hi");
  });

  it("renders blob with pretty byte count", () => {
    // 4 bytes base64 → "AQIDBA==" (padding "==") = 4 bytes
    const blob = { $blob_base64: "AQIDBA==" };
    expect(formatValue(blob)).toBe("<blob · 4 B>");
  });

  it("renders zero-byte blob", () => {
    expect(formatValue({ $blob_base64: "" })).toBe("<blob · 0 B>");
  });

  it("renders truncated blob with size", () => {
    expect(
      formatValue({ $blob_base64_truncated: "xxx", $blob_size: 2_048_000 }),
    ).toBe("<blob · 2.0 MB · preview>");
  });

  it("renders $int64 tagged integer exactly", () => {
    expect(formatValue({ $int64: "9007199254740993" })).toBe(
      "9007199254740993",
    );
  });

  it("renders $real non-finite as its label", () => {
    expect(formatValue({ $real: "NaN" })).toBe("NaN");
    expect(formatValue({ $real: "Infinity" })).toBe("Infinity");
    expect(formatValue({ $real: "-Infinity" })).toBe("-Infinity");
  });
});
