"use client";

import { useEffect, useState } from "react";

import { hosts, script, stillStep } from "@/lib/demo";

const Match = ({ text, query }: { text: string; query: string }) => {
  const at = query ? text.toLowerCase().indexOf(query.toLowerCase()) : -1;

  if (at < 0) {
    return text;
  }

  return (
    <>
      {text.slice(0, at)}
      <b className="font-bold">{text.slice(at, at + query.length)}</b>
      {text.slice(at + query.length)}
    </>
  );
};

const useStep = () => {
  const [index, setIndex] = useState<number | null>(null);

  useEffect(() => {
    let current = 0;
    let timer: number | undefined;

    const advance = () => {
      setIndex(current);
      timer = window.setTimeout(() => {
        current = (current + 1) % script.length;
        advance();
      }, script[current].hold);
    };

    if (!window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      advance();
    }

    return () => {
      window.clearTimeout(timer);
    };
  }, []);

  return index === null ? stillStep : script[index];
};

export const TerminalDemo = () => {
  const step = useStep();
  const selected = hosts[step.selected];
  const blankRows = Math.max(0, 6 - step.rows.length);

  return (
    <figure
      aria-label="A recorded sshpick session: typing consul narrows five hosts to two"
      className="overflow-hidden rounded-[20px] border border-border bg-term text-[12px] leading-[1.7] text-term-fg sm:rounded-[24px] sm:text-[13px]"
    >
      <div className="flex items-center gap-1.5 border-b border-term-rule px-4 py-2.5">
        <span className="size-2.5 rounded-full bg-term-rule" />
        <span className="size-2.5 rounded-full bg-term-rule" />
        <span className="size-2.5 rounded-full bg-term-rule" />
        <span className="ml-3 text-term-muted">~/work $ sshpick</span>
      </div>
      <div className="p-3 sm:p-4" aria-hidden>
        <div className="overflow-hidden rounded-lg border border-term-rule">
          <div className="flex justify-between gap-4 bg-term-strip px-3 py-1">
            <span className={step.query ? "" : "text-term-muted"}>
              {step.query || "Search host, ENV or what"}
              <span className="caret ml-px inline-block h-[1.1em] w-[0.55em] translate-y-[0.2em] bg-term-fg/80" />
            </span>
            <span className="shrink-0 text-term-muted tabular-nums">
              {step.rows.length} hosts
            </span>
          </div>
          <ul className="px-1 py-2">
            {step.rows.map((name) => {
              const host = hosts[name];
              const on = name === step.selected;

              return (
                <li
                  key={name}
                  className={`flex gap-3 rounded px-2 ${on ? "bg-accent text-accent-fg" : ""}`}
                >
                  <span className="w-[15ch] shrink-0">
                    <Match text={host.name} query={step.query} />
                  </span>
                  <span
                    className={`min-w-0 flex-1 truncate ${on ? "" : "text-term-muted"}`}
                  >
                    <Match text={host.what} query={step.query} />
                  </span>
                  <span
                    className={`hidden shrink-0 sm:inline ${on ? "" : "text-term-muted"}`}
                  >
                    {host.env}
                  </span>
                </li>
              );
            })}
            {Array.from({ length: blankRows }, (_, i) => (
              <li key={`blank-${i}`}>&nbsp;</li>
            ))}
          </ul>
          <div className="flex justify-between gap-4 border-t border-term-rule px-3 py-1">
            <span className="min-w-0 truncate text-term-muted">
              {selected.route}
            </span>
            <span className="hidden shrink-0 gap-4 sm:flex">
              <span>
                Keys <kbd className="text-term-muted">?</kbd>
              </span>
              <span>
                Connect <kbd className="text-term-muted">↵</kbd>
              </span>
            </span>
          </div>
        </div>
      </div>
    </figure>
  );
};
