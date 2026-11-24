import { defineConfig } from "oxlint";
import antiSlop from "ultracite/oxlint/anti-slop";
import core from "ultracite/oxlint/core";
import { jsPluginSettings, selectJsPlugins } from "ultracite/oxlint/js-plugins";
import landmines from "ultracite/oxlint/landmines";
import next from "ultracite/oxlint/next";
import nextJsPlugins from "ultracite/oxlint/next/js-plugins";
import react from "ultracite/oxlint/react";

import { outOfScope } from "./oxfmt.config.ts";

const jsPlugins = selectJsPlugins(["github", "sonarjs", "react-doctor"]);

export default defineConfig({
  env: core.env,
  extends: [core, react, next, nextJsPlugins, antiSlop, landmines, jsPlugins],
  ignorePatterns: [...(core.ignorePatterns ?? []), ...outOfScope],
  jsPlugins: [...jsPlugins.jsPlugins, ...landmines.jsPlugins],
  options: { typeAware: true },
  overrides: [
    {
      files: ["**/*.ts", "**/*.tsx"],
      rules: {
        "sonarjs/function-name": "off",
        "sonarjs/no-duplicate-string": "off",
      },
    },
    {
      files: ["apps/web/lib/markdown.ts", "apps/web/lib/repo.ts"],
      rules: { "sonarjs/super-linear-regex": "off" },
    },
    {
      files: ["apps/web/app/docs/**/page.tsx"],
      rules: { "react/no-danger": "off" },
    },
  ],
  settings: jsPluginSettings,
});
