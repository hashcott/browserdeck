import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import { AppFrame } from "./AppFrame";

test("a toast shown over Settings is in the output, outside the hidden editor, and there is one toast region", () => {
  const html = renderToStaticMarkup(
    <AppFrame view="settings" toast="A button press from the phone failed: OBS is not connected." editor={<p>editor</p>}>
      <main>settings</main>
    </AppFrame>);
  expect(html).toContain("OBS is not connected.");
  expect(html.match(/class="toast"/g)).toHaveLength(1);
  // The editor wrapper is the only element with `hidden`; the toast must sit after it, not inside it.
  const hidden = /<div hidden="">.*?<\/div>/.exec(html)![0];
  expect(html.match(/hidden=""/g)).toHaveLength(1);
  expect(hidden).toBe('<div hidden=""><p>editor</p></div>');
  expect(html.slice(html.indexOf(hidden) + hidden.length)).toContain('class="toast" role="status" aria-live="polite"><span>');
});
