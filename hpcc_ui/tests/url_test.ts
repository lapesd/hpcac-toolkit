import { assertEq } from "https://deno.land/x/test_utils@v1.0.0/mod.ts";

Deno.test("url test", () => {
  const url = new URL("./foo.js", "https://deno.land/");
  assertEq(url.href, "https://deno.land/foo.js");
});
