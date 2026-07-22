import { readdir, readFile } from "node:fs/promises";

const dist = new URL("../dist/", import.meta.url);
const declarations = (await readdir(dist, { recursive: true })).filter(path =>
  path.endsWith(".d.ts"),
);

for (const declaration of declarations) {
  const source = await readFile(new URL(declaration, dist), "utf8");
  if (source.includes("@blueshift-gg/quasar-svm")) {
    throw new Error(`${declaration} exposes the private QuasarSVM backend`);
  }
}
