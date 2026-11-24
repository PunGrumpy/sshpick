# web

## Purpose

The sshpick website: a landing page and the user documentation, built from the files in this
repository so it never says something the README or `docs` do not. It is a static export and is
not deployed from here.

## Architecture

- `lib/repo.ts` reads the repository at build time: the version from
  `crates/sshpick/package.json`, the license from `Cargo.toml`, the intro, install command and
  command examples from `README.md`, and the page list from `docs/README.md`. A missing piece
  fails the build.
- `lib/markdown.ts` renders `docs/*.md` with `marked`, gives headings GitHub's anchors, and turns
  `foo.md#x` links into `/docs/foo/#x`. Other relative links go to the file on GitHub.
- `app/page.tsx` is the landing page. `app/docs/[slug]/page.tsx` is one page per `docs/*.md`,
  from `generateStaticParams`, and `app/docs/page.tsx` is `docs/README.md`.
- `lib/demo.ts` is the terminal demo as a table of frames, each with how long it holds.
  `components/terminal-demo.tsx` replays it. Its hosts are those of
  `crates/sshpick/tests/fixtures/ui`, and the build fails if one is missing there.
- `components/motion.tsx` turns on Lenis smooth scrolling and the scroll reveals.

## Notes

- `bunx turbo run build --filter=web` writes the site to `out/`; any static file server can serve
  it. `bun run --filter web dev` runs it locally.
- Under `prefers-reduced-motion` there is no smooth scrolling, no reveal and no replay: the demo
  shows one still frame.
- The colours are tokens in `app/globals.css`, light and dark from the system. The accent is the
  picker's selection blue.
- The site is private and Changesets ignores it; it has no version of its own.
