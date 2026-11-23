export type HostName =
  | "lab-a"
  | "lab-b"
  | "newstg-consul1"
  | "newstg-consul2"
  | "newstg-bastion";

export interface Host {
  name: HostName;
  what: string;
  env: string;
  route: string;
}

export const hosts: Record<HostName, Host> = {
  "lab-a": {
    name: "lab-a",
    what: "LAB — personal VMs",
    env: "LAB",
    route: "ubuntu@10.11.212.191:22 via newstg-bastion",
  },
  "lab-b": {
    name: "lab-b",
    what: "build box",
    env: "LAB",
    route: "ubuntu@10.11.212.192:22 via newstg-bastion",
  },
  "newstg-consul1": {
    name: "newstg-consul1",
    what: "consul server 1 — mgmt",
    env: "NEW STAGING",
    route: "ubuntu@192.0.2.11:22 via newstg-bastion",
  },
  "newstg-consul2": {
    name: "newstg-consul2",
    what: "",
    env: "NEW STAGING",
    route: "ubuntu@192.0.2.12:22 via newstg-bastion",
  },
  "newstg-bastion": {
    name: "newstg-bastion",
    what: "bastion",
    env: "NEW STAGING",
    route: "ubuntu@192.0.2.10:22",
  },
};

export interface Step {
  hold: number;
  query: string;
  rows: HostName[];
  selected: HostName;
}

const all: HostName[] = [
  "lab-a",
  "lab-b",
  "newstg-consul1",
  "newstg-consul2",
  "newstg-bastion",
];

const consul: HostName[] = ["newstg-consul1", "newstg-consul2"];

const typing = (word: string, rows: HostName[], selected: HostName) =>
  Array.from({ length: word.length }, (_, i) => ({
    hold: 140,
    query: word.slice(0, i + 1),
    rows,
    selected,
  }));

/**
 * A scripted picker session over the hosts in crates/sshpick/tests/fixtures/ui, the fixture the
 * frame tests and the README picture use. Each step is one frame and how long it stays up.
 */
export const script: Step[] = [
  { hold: 1800, query: "", rows: all, selected: "lab-a" },
  { hold: 700, query: "", rows: all, selected: "lab-b" },
  { hold: 900, query: "", rows: all, selected: "newstg-consul1" },
  ...typing("consul", consul, "newstg-consul1"),
  { hold: 1400, query: "consul", rows: consul, selected: "newstg-consul1" },
  { hold: 2200, query: "consul", rows: consul, selected: "newstg-consul2" },
  { hold: 600, query: "consu", rows: consul, selected: "newstg-consul1" },
  { hold: 90, query: "cons", rows: consul, selected: "newstg-consul1" },
  { hold: 90, query: "con", rows: consul, selected: "newstg-consul1" },
  { hold: 90, query: "co", rows: consul, selected: "newstg-consul1" },
  { hold: 90, query: "c", rows: consul, selected: "newstg-consul1" },
];

/** The frame shown when motion is reduced: the narrowed list with the second host selected. */
export const stillStep: Step = {
  hold: 0,
  query: "consul",
  rows: consul,
  selected: "newstg-consul2",
};
