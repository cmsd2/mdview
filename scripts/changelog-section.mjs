// Prints the CHANGELOG.md section for one version, for use as the GitHub
// release description. The version comes from the first argument, or from
// GITHUB_REF_NAME (the pushed tag, e.g. "v0.2.0") in the release workflow.
// Exits non-zero when the section is missing or empty, so a release can't go
// out without notes.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = (process.argv[2] ?? process.env.GITHUB_REF_NAME ?? "").replace(/^v/, "");
if (!version) {
  console.error("usage: changelog-section.mjs <version>  (or set GITHUB_REF_NAME)");
  process.exit(2);
}

const lines = readFileSync(join(root, "CHANGELOG.md"), "utf8").split("\n");
const heading = new RegExp(`^## \\[${version.replace(/\./g, "\\.")}\\]`);
const start = lines.findIndex((l) => heading.test(l));
if (start === -1) {
  console.error(`CHANGELOG.md has no "## [${version}]" section`);
  process.exit(1);
}
// The section runs to the next "## [" heading or the link references.
let end = lines.findIndex((l, i) => i > start && (/^## \[/.test(l) || /^\[[^\]]+\]: /.test(l)));
if (end === -1) end = lines.length;

const body = lines.slice(start + 1, end).join("\n").trim();
if (!body) {
  console.error(`CHANGELOG.md section for ${version} is empty`);
  process.exit(1);
}
console.log(body);
