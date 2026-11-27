# vendor/stageshow

## Purpose

The part of the Stageshow kit that `components/picker-demo.tsx` is built on:
`Stage` (design size, scale-to-fit, the scripted scene, reduced motion) and
`Window` (real window chrome, `chrome="terminal"` included). Vendored rather
than depended on because the kit ships inside a skill, not as a package.

## Architecture

- `stage.tsx`, `act.ts`, `clock.ts`, `cursor.tsx`, `cursors.tsx`, `keys.tsx`,
  `measure.ts`, `reduced-motion.ts`, `looks.ts`, `ease.ts`, `util.ts`: the
  stage and the script API.
- `window.tsx`: the window chromes.
- `index.ts` re-exports only what the demo uses.
- Each component imports its own CSS; `stage.css` holds the `--ss-*` tokens.

## Notes

- Upstream and unmodified, so the repository's formatter and linter skip it
  (`oxfmt.config.ts`). Update it by copying the files across again.
- MIT, see `LICENSE`.
