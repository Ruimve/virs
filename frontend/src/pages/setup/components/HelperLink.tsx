import { memo } from 'react';

interface HelperLinkProps {
  href: string;
  children: string;
}

export const HelperLink = memo(({ href, children }: HelperLinkProps) => (
  <a
    href={href}
    target="_blank"
    rel="noopener noreferrer"
    className="inline-flex items-center gap-1 text-xs text-accent no-underline mt-3 transition-opacity hover:opacity-80"
  >
    {children}
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} className="w-3 h-3">
      <path d="M7 17L17 7M7 7h10v10" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  </a>
));
