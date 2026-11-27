# web

## Purpose

The keyway website: a landing page and the user documentation, built from the files in this
repository so it never says something the README or `docs` do not. It is a static export and is
not deployed from here.

## Architecture

- `lib/repo.ts` reads the repository at build time: the version from
  `crates/keyway/package.json`, the license from `Cargo.toml`, the intro, install command and
  command examples from `README.md`, and the page list from `docs/README.md`. A missing piece
  fails the build.
- `lib/markdown.ts` renders `docs/*.md` with `marked`, gives headings GitHub's anchors, and turns
  `foo.md#x` links into `/docs/foo/#x`. Other relative links go to the file on GitHub.
- `app/page.tsx` is the landing page. `app/docs/[slug]/page.tsx` is one page per `docs/*.md`,
  from `generateStaticParams`, and `app/docs/page.tsx` is `docs/README.md`.
- `lib/demo.ts` is the hero demo's content: the hosts, how a query narrows them, the footer's
  keys and the shell session. Its hosts are those of `crates/keyway/tests/fixtures/ui`, and the
  build fails if one is missing there.
- `components/picker-demo.tsx` plays that session on a Stageshow stage from `vendor/stageshow`.
  `components/REHEARSAL.md` records what was filmed.
- `components/motion.tsx` turns on Lenis smooth scrolling and the scroll reveals, and does
  nothing under `prefers-reduced-motion`.

## Notes

- `bunx turbo run build --filter=web` writes the site to `out/`; any static file server can serve
  it. `bun run --filter web dev` runs it locally.
- Under `prefers-reduced-motion` there is no smooth scrolling, no reveal and no replay: the demo
  shows one still frame, and nothing is hidden waiting to be scrolled to.
- The page is one family, Geist Mono, two inks and hairlines, light and dark from the system;
  the tokens are in `app/globals.css`. The terminal keeps the picker's own colours instead: the
  xterm grey ramp and ANSI blue, in `components/picker-demo.css`.
- The site is private and Changesets ignores it; it has no version of its own.
