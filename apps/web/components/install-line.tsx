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
    <div className="flex items-start gap-3 rounded-[10px] border border-hair bg-card px-4 py-3 text-[13px]">
      <code className="min-w-0 flex-1 break-all">
        <span className="text-muted select-none">$ </span>
        {command}
      </code>
      <button
        aria-live="polite"
        className="press -my-1 -mr-2 min-w-[7ch] shrink-0 cursor-pointer rounded-full px-2.5 py-1 text-right text-muted transition-colors hover:text-fg focus-visible:text-fg"
        onClick={() => {
          void copy();
        }}
        type="button"
      >
        {copied ? "Copied" : "Copy"}
      </button>
    </div>
  );
};
