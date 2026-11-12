import { expect, test } from "vitest";
import { formatArgs, parseArgs } from "./args";

test("parseArgs splits on whitespace and honours double quotes", () => {
  expect(parseArgs('  a  b\tc ')).toEqual(["a", "b", "c"]);
  expect(parseArgs('--name "My Profile" x')).toEqual(["--name", "My Profile", "x"]);
  expect(parseArgs('say "he said \\"hi\\""')).toEqual(["say", 'he said "hi"']);
  expect(parseArgs('a ""')).toEqual(["a", ""]);
  expect(parseArgs('a "unterminated rest')).toEqual(["a", "unterminated rest"]);
  expect(parseArgs("")).toEqual([]);
});
test("formatArgs quotes empty, spaced and quoted arguments only", () => {
  expect(formatArgs(["a", "b c", "", 'q"x'])).toBe('a "b c" "" "q\\"x"');
});
test("parseArgs(formatArgs(a)) round-trips", () => {
  const cases = [
    [], [""], ["a"], ["a b", "c"], ['"'], ['a"b'], ["  lead", "trail  "], ["\\"], ["a b\\"], ['\\"'], ['a\\\\"b c'],
    ["C:\\Program Files\\x\\", "y"], ["", "", "x"], ["tab\there"], ["new\nline"], ['"quoted"', "'single'"], ["é ü", "🚀 go"],
  ];
  for (const a of cases) expect(parseArgs(formatArgs(a))).toEqual(a);
  // pseudo-random strings over a nasty alphabet
  const alpha = ['a', ' ', '"', '\\', '\t', 'b'];
  let seed = 7;
  const rnd = () => (seed = (seed * 1103515245 + 12345) & 0x7fffffff);
  for (let n = 0; n < 500; n++) {
    const arr = Array.from({ length: rnd() % 4 }, () => Array.from({ length: rnd() % 6 }, () => alpha[rnd() % alpha.length]).join(""));
    expect(parseArgs(formatArgs(arr))).toEqual(arr);
  }
});
