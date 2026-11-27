// The subset of the Stageshow kit the hero demo needs: the stage, its script
// and the window chrome. The parts that draw GUI lists, phones and charts are
// left out, so nothing here pulls in an icon set.
export type { Act, Script } from './act'
export { clock } from './clock'
export { bezier, css as easeCss, EASE } from './ease'
export { useLook } from './looks'
export { useReducedMotion } from './reduced-motion'
export { Stage } from './stage'
export type { Play, StageProps } from './stage'
export { Bar, Caret, Prompt, Text, Window } from './window'
export type { WindowProps } from './window'
