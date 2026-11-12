import { expect, test } from "vitest";
import { numFieldBlur, numFieldInput, textFieldBlur } from "./fieldRules";

test("numFieldInput commits only a plain whole number inside the range", () => {
  expect(numFieldInput("", 1, 8)).toBeNull();
  expect(numFieldInput("abc", 1, 8)).toBeNull();
  expect(numFieldInput("4a", 1, 8)).toBeNull();
  expect(numFieldInput("-3", 1, 8)).toBeNull();
  expect(numFieldInput("3.5", 1, 8)).toBeNull();
  expect(numFieldInput("0", 1, 8)).toBeNull();
  expect(numFieldInput("9", 1, 8)).toBeNull();
  expect(numFieldInput("123456", 1, 65535)).toBeNull();
  expect(numFieldInput("007", 1, 8)).toBe(7);
  expect(numFieldInput("1", 1, 8)).toBe(1);
  expect(numFieldInput("8", 1, 8)).toBe(8);
  expect(numFieldInput("65535", 1, 65535)).toBe(65535);
  expect(numFieldInput("65536", 1, 65535)).toBeNull();
});
test("numFieldBlur commits a changed valid value and otherwise shows the saved one", () => {
  expect(numFieldBlur("5", 4, 1, 8)).toEqual({ commit: 5, show: "5" });
  expect(numFieldBlur("4", 4, 1, 8)).toEqual({ commit: null, show: "4" });
  expect(numFieldBlur("004", 4, 1, 8)).toEqual({ commit: null, show: "4" });
  expect(numFieldBlur("", 4, 1, 8)).toEqual({ commit: null, show: "4" });
  expect(numFieldBlur("99", 4, 1, 8)).toEqual({ commit: null, show: "4" });
  expect(numFieldBlur("x", 4, 1, 8)).toEqual({ commit: null, show: "4" });
});
test("textFieldBlur: a host is never empty, a password may be", () => {
  expect(textFieldBlur("  ", "localhost", false)).toEqual({ commit: null, show: "localhost" });
  expect(textFieldBlur("", "localhost", false)).toEqual({ commit: null, show: "localhost" });
  expect(textFieldBlur(" pc ", "localhost", false)).toEqual({ commit: "pc", show: "pc" });
  expect(textFieldBlur("localhost", "localhost", false)).toEqual({ commit: null, show: "localhost" });
  expect(textFieldBlur("", "secret", true)).toEqual({ commit: "", show: "" });
  expect(textFieldBlur("secret", "secret", true)).toEqual({ commit: null, show: "secret" });
});
