"use client";

import {
  caps,
  columns,
  counter,
  edges,
  hosts,
  matching,
  prompt,
  ROOM,
  runs,
  session,
} from "@/lib/demo";
import type { HostName } from "@/lib/demo";
import type { Act } from "@/vendor/stageshow";
import { Prompt, Stage, Window } from "@/vendor/stageshow";

import "./picker-demo.css";

interface Scene {
  query: string;
  index: number;
  screen: "picker" | "shell";
  shell: "remote" | "local";
  typed: string;
}

type Vars = React.CSSProperties & Record<`--${string}`, string>;

const START: Scene = {
  query: "",
  index: 0,
  screen: "picker",
  shell: "remote",
  typed: "",
};

const POSTER: Scene = { ...START, query: "consul", index: 1 };

const WIDTHS: Vars = {
  "--kw-host": `${columns.host}ch`,
  "--kw-last": `${columns.last}ch`,
  "--kw-env": `${columns.env}ch`,
};

const script = async (a: Act<Scene>) => {
  await a.wait(1700);
  await a.type("query", "consul", { interval: 115 });
  a.mark("narrowed");
  await a.wait(1400);
  await a.set({ index: 1 });
  a.mark("selected");
  await a.wait(1600);
  await a.set({ screen: "shell", shell: "remote", typed: "" });
  a.mark("connected");
  await a.wait(2200);
  await a.type("typed", "exit", { interval: 110 });
  await a.wait(260);
  await a.set({ shell: "local", typed: "" });
  await a.wait(900);
  await a.type("typed", session.command, { interval: 110 });
  await a.wait(260);
  await a.set(START);
};

const Cell = ({ text, query }: { text: string; query: string }) => (
  <>
    {runs(text, query).map((run) =>
      run.matched ? (
        <b className="kw-hit" key={run.text}>
          {run.text}
        </b>
      ) : (
        run.text
      )
    )}
  </>
);

const Line = ({
  children,
  className,
  on,
}: {
  children?: React.ReactNode;
  className?: string;
  on?: boolean;
}) => (
  <div className="kw-line">
    <span className="kw-side">│</span>
    <span
      className={`kw-bar ${className ?? ""}`}
      data-on={on === true ? "" : undefined}
    >
      {children}
    </span>
    <span className="kw-side">│</span>
  </div>
);

const Row = ({
  name,
  query,
  on,
}: {
  name: HostName;
  query: string;
  on: boolean;
}) => {
  const host = hosts[name];

  return (
    <Line className="kw-row" on={on}>
      <span className="kw-cell kw-host">
        <Cell query={query} text={host.name} />
      </span>
      <span className="kw-cell kw-what">
        <Cell query={query} text={host.what} />
      </span>
      <span className="kw-cell kw-last">{host.last}</span>
      <span className="kw-cell kw-env">
        <Cell query={query} text={host.env} />
      </span>
    </Line>
  );
};

const Picker = ({
  scene,
  rows,
  wide,
}: {
  scene: Scene;
  rows: HostName[];
  wide: boolean;
}) => {
  const selected = hosts[rows[scene.index] ?? rows[0]];
  const blanks = Math.max(0, ROOM - rows.length);

  return (
    <div className="kw-panel" style={WIDTHS}>
      <div className="kw-edge">{edges.top}</div>
      <Line className="kw-field">
        <span className="kw-typed" data-typing>
          {scene.query === "" ? (
            <span className="kw-placeholder">
              <span className="kw-cursor">{prompt.slice(0, 1)}</span>
              {prompt.slice(1)}
            </span>
          ) : (
            <>
              {scene.query}
              <span className="kw-cursor"> </span>
            </>
          )}
        </span>
        <span className="kw-count">{counter(scene.query)}</span>
      </Line>
      <Line />
      {rows.map((name, at) => (
        <Row
          key={name}
          name={name}
          on={at === scene.index}
          query={scene.query}
        />
      ))}
      {Array.from({ length: blanks }, (_, at) => (
        <Line key={`blank-${at}`} />
      ))}
      <div className="kw-edge">{edges.mid}</div>
      <Line className="kw-hint">
        <span className="kw-route">{selected.route}</span>
        <span className="kw-caps">
          {caps(scene.query, wide).map((cap) => (
            <span key={cap.label}>
              {cap.label} <kbd>{cap.key}</kbd>
            </span>
          ))}
        </span>
      </Line>
      <div className="kw-edge">{edges.bottom}</div>
    </div>
  );
};

const Shell = ({ scene }: { scene: Scene }) => (
  <div className="kw-shell">
    <Prompt path={session.path}>{session.before.command}</Prompt>
    <div className="dim">{session.before.out}</div>
    <Prompt path={session.path}>{session.command}</Prompt>
    <div className="dim">{session.login}</div>
    <div className="kw-remote">
      {session.remote} {scene.shell === "remote" ? scene.typed : "exit"}
      {scene.shell === "remote" && <span className="kw-cursor"> </span>}
    </div>
    {scene.shell === "local" && (
      <>
        <div className="dim">{session.logout}</div>
        <div className="dim">{session.closed}</div>
        <Prompt path={session.path}>
          {scene.typed}
          <span className="kw-cursor"> </span>
        </Prompt>
      </>
    )}
  </div>
);

/**
 * A keyway session replayed in a terminal: the list, a query that narrows it, the route the
 * selected host resolves to, and the shell once Enter has handed the terminal to ssh. The picker
 * is a keyboard app, so the script types and the terminal redraws, as it does.
 */
export const PickerDemo = () => (
  <Stage
    className="kw-stage"
    compact={{ below: 900, width: 358, height: 190 }}
    height={304}
    initial={START}
    poster={POSTER}
    script={script}
    theme="dark"
    width={1000}
  >
    {(scene, _rect, layout) => (
      <Window
        body={{ className: "kw-body", "data-crop": "" }}
        chrome="terminal"
        className="kw-term"
        lights="grey"
        title={layout === "wide" ? "work — zsh" : undefined}
      >
        {scene.screen === "picker" ? (
          <Picker
            rows={matching(scene.query)}
            scene={scene}
            wide={layout === "wide"}
          />
        ) : (
          <Shell scene={scene} />
        )}
      </Window>
    )}
  </Stage>
);
