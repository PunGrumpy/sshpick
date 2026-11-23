import { defineConfig } from "oxfmt";
import ultracite from "ultracite/oxfmt";

export const outOfScope = [
  "/.changeset/",
  "/.github/",
  "/assets/",
  "/crates/",
  "/docs/",
  "/integrations/",
  "/skills/",
  "/target/",
  "**/*.md",
  "**/*.toml",
];

export default defineConfig({
  ...ultracite,
  ignorePatterns: [...(ultracite.ignorePatterns ?? []), ...outOfScope],
  sortTailwindcss: {
    ...ultracite.sortTailwindcss,
    stylesheet: "./apps/web/app/globals.css",
  },
});
