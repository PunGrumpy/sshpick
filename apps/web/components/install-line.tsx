"use client";

import { useEffect, useState } from "react";

export const InstallLine = ({ command }: { command: string }) => {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let timer: number | undefined;

    if (copied) {
      timer = window.setTimeout(() => {
        setCopied(false);
      }, 1600);
    }

    return () => {
      window.clearTimeout(timer);
    };
  }, [copied]);

  const copy = async () => {
    await navigator.clipboard.writeText(command);
    setCopied(true);
  };

  return (
    <div className="flex items-start gap-3 rounded-lg border border-border bg-card px-4 py-3 text-[13px]">
      <code className="min-w-0 flex-1 break-all">
        <span className="text-muted select-none">$ </span>
        {command}
      </code>
      <button
        type="button"
        onClick={() => {
          void copy();
        }}
        className="press -my-1.5 -mr-2 min-w-[6ch] shrink-0 cursor-pointer rounded-md px-2 py-1.5 text-right text-muted hover:text-fg focus-visible:text-fg"
        aria-live="polite"
      >
        {copied ? "copied" : "copy"}
      </button>
    </div>
  );
};
