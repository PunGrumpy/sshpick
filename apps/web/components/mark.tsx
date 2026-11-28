export const Mark = ({ className }: { className?: string }) => (
  <svg aria-hidden className={className} viewBox="0 0 24 24">
    <mask id="keyway-slot">
      <rect fill="#fff" height="24" width="24" />
      <path
        d="M12 0v9l1.5 1.5-3 3L12 15v9"
        fill="none"
        stroke="#000"
        strokeWidth="3"
      />
    </mask>
    <circle
      cx="12"
      cy="12"
      fill="currentColor"
      mask="url(#keyway-slot)"
      r="10"
    />
  </svg>
);
