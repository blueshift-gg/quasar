import { readdir, readFile } from "node:fs/promises";

const dist = new URL("../dist/", import.meta.url);
const declarations = (await readdir(dist, { recursive: true })).filter(path =>
  path.endsWith(".d.ts"),
);

// Match an actual module specifier for the private backend (a quoted
// `litesvm` or `litesvm/...` in an import/export/require), not the bare word,
// which legitimately appears in prose and doc comments.
const backendSpecifier = /["']litesvm(?:\/[^"']*)?["']/;

for (const declaration of declarations) {
  const source = await readFile(new URL(declaration, dist), "utf8");
  if (backendSpecifier.test(source)) {
    throw new Error(`${declaration} exposes the private LiteSVM backend`);
  }
}
