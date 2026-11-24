import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

import manifest from "../../../crates/sshpick/package.json";

const repoRoot = path.resolve(process.cwd(), "../..");
const read = (file: string) => readFileSync(path.join(repoRoot, file), "utf-8");

const joinLines = (text: string) => text.replaceAll(/\s*\n\s*/gu, " ");

export const repoUrl = "https://github.com/PunGrumpy/sshpick";
export const blobUrl = (file: string) => `${repoUrl}/blob/main/${file}`;

const expectMatch = (
  file: string,
  what: string,
  match: RegExpMatchArray | null
) => {
  if (!match) {
    throw new Error(
      `${file}: cannot find ${what}; the site reads it at build time`
    );
  }

  return match;
};

export const { version } = manifest;

export const license = () =>
  expectMatch(
    "Cargo.toml",
    "license",
    /(?<=^license = ")[^"]+(?="$)/mu.exec(read("Cargo.toml"))
  )[0];

const paragraphAfter = (text: string, marker: string, file: string) => {
  const at = text.indexOf(marker);

  if (at === -1) {
    throw new Error(`${file}: cannot find ${JSON.stringify(marker)}`);
  }

  const rest = text.slice(at + marker.length).replace(/^\n+/u, "");

  return joinLines(rest.slice(0, rest.indexOf("\n\n")));
};

export const readme = () => {
  const file = "README.md";
  const text = read(file);

  const intro = joinLines(
    expectMatch(
      file,
      "the intro",
      /^sshpick is [\s\S]+?\n\n/mu.exec(text)
    )[0].trim()
  );

  const platforms = expectMatch(
    file,
    "the platforms",
    /(?<=runs on )[^.]+(?=\.)/u.exec(intro)
  )[0]
    .split(" and ")
    .join(", ");

  const [installLine] = expectMatch(
    file,
    "the install command",
    /^cargo install .*--tag v\S+ sshpick$/mu.exec(text)
  );

  const install = installLine.replace(/--tag v\S+/u, `--tag v${version}`);

  const [block] = expectMatch(
    file,
    "the command examples",
    /(?<=```bash\n)(?:sshpick[^\n]*\n)+(?=```)/u.exec(text)
  );

  const commands = block
    .trimEnd()
    .split("\n")
    .map((line) => {
      const [command, note = ""] = line.split(/\s+#\s+/u);

      return { command: command.trim(), note: note.trim() };
    });

  const picker = paragraphAfter(
    text.slice(text.indexOf("```text")),
    "```\n",
    file
  );

  const agents = paragraphAfter(text, "## For coding agents\n", file);

  return { intro, platforms, install, commands, picker, agents };
};

export interface DocPage {
  slug: string;
  title: string;
  summary: string;
}

/** The order and one-line summaries come from docs/README.md, so the sidebar matches it. */
export const docPages = () => {
  const pages: DocPage[] = [];

  for (const { groups } of read("docs/README.md").matchAll(
    /^- \[(?<title>[^\]]+)\]\((?<slug>[a-z-]+)\.md\): (?<summary>[\s\S]+?)(?=\n- |\n*(?![\s\S]))/gmu
  )) {
    if (groups) {
      pages.push({
        title: groups.title,
        slug: groups.slug,
        summary: joinLines(groups.summary),
      });
    }
  }

  const missing = readdirSync(path.join(repoRoot, "docs")).flatMap((name) => {
    const slug = name.slice(0, -3);

    return name.endsWith(".md") &&
      name !== "README.md" &&
      !pages.some((page) => page.slug === slug)
      ? [slug]
      : [];
  });

  if (missing.length > 0) {
    throw new Error(`docs/README.md does not list ${missing.join(", ")}`);
  }

  return pages;
};

export const docSource = (slug: string) =>
  read(slug === "" ? "docs/README.md" : `docs/${slug}.md`);

/**
 * The demo replays hosts from the frame-test fixture; a renamed fixture host fails the build
 * instead of leaving the site showing hosts the tests no longer use.
 */
export const assertFixtureHosts = (names: string[]) => {
  const dir = "crates/sshpick/tests/fixtures/ui/.ssh/config.d";

  const declared = new Set(
    readdirSync(path.join(repoRoot, dir)).flatMap((name) =>
      [...read(`${dir}/${name}`).matchAll(/^Host\s+(?<names>.+)$/gmu)].flatMap(
        (match) => match.groups?.names.split(/\s+/u) ?? []
      )
    )
  );

  const unknown = names.filter((name) => !declared.has(name));

  if (unknown.length > 0) {
    throw new Error(`${dir} has no host ${unknown.join(", ")}`);
  }
};
