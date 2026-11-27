import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

import manifest from "../crates/keyway/package.json";

interface Rule {
  file: string;
  pattern: RegExp;
  expectedMatches: number;
}

const root = path.resolve(import.meta.dir, "..");

const read = (file: string) => readFileSync(path.join(root, file), "utf-8");

const { version } = manifest;

if (!/^\d+\.\d+\.\d+$/u.test(version)) {
  throw new Error(
    `crates/keyway/package.json: version ${version} is not X.Y.Z`
  );
}

const lockPackages = ["keyway", "keyway-cli", "keyway-core", "keyway-ui"];

const rules: Rule[] = [
  {
    file: "Cargo.toml",
    pattern: /(?<=\[workspace\.package\]\nversion = ")[^"]+(?=")/gu,
    expectedMatches: 1,
  },
  {
    file: "Cargo.lock",
    pattern: new RegExp(
      `(?<=\\nname = "(?:${lockPackages.join("|")})"\\nversion = ")[^"]+(?=")`,
      "gu"
    ),
    expectedMatches: lockPackages.length,
  },
  {
    file: "README.md",
    pattern:
      /(?<=--git https:\/\/github\.com\/PunGrumpy\/keyway --tag v)\S+(?= keyway)/gu,
    expectedMatches: 1,
  },
];

for (const { file, pattern, expectedMatches } of rules) {
  const before = read(file);
  const found = before.match(pattern)?.length ?? 0;

  if (found !== expectedMatches) {
    throw new Error(
      `${file}: expected ${expectedMatches} version line(s), found ${found}`
    );
  }

  const after = before.replaceAll(pattern, version);

  if (after !== before) {
    writeFileSync(path.join(root, file), after);
    console.log(`${file}: ${version}`);
  }
}
