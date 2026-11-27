/**
 * The hosts the hero demo replays, from `crates/keyway/tests/fixtures/ui`, the fixture the frame
 * tests and the README picture use. `assertFixtureHosts` fails the build if one is renamed there.
 */

export type HostName =
  | "lab-a"
  | "lab-b"
  | "newstg-bastion"
  | "newstg-consul1"
  | "newstg-consul2";

export interface Host {
  name: HostName;
  /** The comment above the Host line, the picker's WHAT column. */
  what: string;
  /** The section comment, the picker's ENV column. */
  env: string;
  /** How long ago it was last connected to, the picker's LAST column. */
  last: string;
  /** Where `ssh -G` says it goes, the line under the selected host. */
  route: string;
}

export const hosts: Record<HostName, Host> = {
  "lab-a": {
    name: "lab-a",
    what: "LAB — personal VMs",
    env: "LAB",
    last: "2h",
    route: "ubuntu@10.11.212.191:22 via newstg-bastion",
  },
  "lab-b": {
    name: "lab-b",
    what: "build box",
    env: "LAB",
    last: "",
    route: "ubuntu@10.11.212.192:22 via newstg-bastion",
  },
  "newstg-consul1": {
    name: "newstg-consul1",
    what: "consul server 1 — mgmt",
    env: "NEW STAGING",
    last: "",
    route: "ubuntu@192.0.2.11:22 via newstg-bastion",
  },
  "newstg-consul2": {
    name: "newstg-consul2",
    what: "",
    env: "NEW STAGING",
    last: "",
    route: "ubuntu@192.0.2.12:22 via newstg-bastion",
  },
  "newstg-bastion": {
    name: "newstg-bastion",
    what: "bastion",
    env: "NEW STAGING",
    last: "",
    route: "ubuntu@192.0.2.10:22",
  },
};

/** Recently used first, then the order the config declares them in. */
export const order: HostName[] = [
  "lab-a",
  "lab-b",
  "newstg-consul1",
  "newstg-consul2",
  "newstg-bastion",
];

/** HOST is as wide as the longest name, and ENV as wide as the longest label. */
export const columns = {
  host: Math.max(...order.map((name) => name.length)),
  env: Math.max(...order.map((name) => hosts[name].env.length)),
  last: 4,
};

/**
 * One word of a host, split where the query matched it. The picker draws a matched run bold, and
 * ranks on a fuzzy match; the demo only ever types a word its hosts contain whole, so a
 * case-insensitive run is the same answer with none of the ranking.
 */
export interface Run {
  text: string;
  matched: boolean;
}

export const runs = (text: string, query: string) => {
  const at = query ? text.toLowerCase().indexOf(query.toLowerCase()) : -1;

  if (at < 0) {
    return [{ text, matched: false }];
  }

  return [
    { text: text.slice(0, at), matched: false },
    { text: text.slice(at, at + query.length), matched: true },
    { text: text.slice(at + query.length), matched: false },
  ].filter((run) => run.text !== "");
};

const hit = (host: Host, query: string) =>
  [host.name, host.what, host.env].some((field) =>
    field.toLowerCase().includes(query.toLowerCase())
  );

/** The hosts a query leaves, in the order the picker lists them. */
export const matching = (query: string) =>
  query === "" ? order : order.filter((name) => hit(hosts[name], query));

/** `5 hosts` with nothing typed, `2 / 5` once a query has narrowed them. */
export const counter = (query: string) => {
  const shown = matching(query).length;

  return query === ""
    ? `${shown} ${shown === 1 ? "host" : "hosts"}`
    : `${shown} / ${order.length}`;
};

export const prompt = "Search host, ENV or what";

/** The widest the picker's box gets, however wide the terminal is. */
export const PANEL_COLS = 110;

/** The rows the list keeps, whatever matches. */
export const ROOM = 5;

/** The box: a rounded top, the rule above the key hints, and a rounded bottom. */
export const edges = {
  top: `╭${"─".repeat(PANEL_COLS - 2)}╮`,
  mid: `├${"─".repeat(PANEL_COLS - 2)}┤`,
  bottom: `╰${"─".repeat(PANEL_COLS - 2)}╯`,
};

/**
 * The footer's keys. `Quit esc` is the first cap a narrow row loses, so a terminal too narrow for
 * the box keeps only the two that are left; `?` becomes `F1` once something is typed, because `?`
 * is a filter character then.
 */
export const caps = (query: string, wide: boolean) => [
  { label: "Keys", key: query === "" ? "?" : "F1" },
  ...(wide ? [{ label: "Quit", key: "esc" }] : []),
  { label: "Connect", key: "↵" },
];

/**
 * What the shell shows once the picker leaves the alternate screen and ssh has the terminal: the
 * work that was there before `keyway` ran, then the login and the remote prompt.
 */
export const session = {
  path: "~/work",
  command: "keyway",
  host: "newstg-consul2",
  before: { command: "git pull --ff-only", out: "Already up to date." },
  login: "Last login: Fri Nov 28 09:12:04 2025",
  remote: "ubuntu@newstg-consul2:~$",
  logout: "logout",
  closed: "Connection to 192.0.2.12 closed.",
} as const;
