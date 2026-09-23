/**
 * What a Connect click that opened nothing says: the desktop browser is
 * probably not installed, so download it first.
 */
import { Download } from 'lucide-react';

/** The not-installed note, with the download link; `className` places it. */
export default function DesktopNotOpened({ className = 'mt-3' }: { /** Placement classes. */ className?: string }) {
  return (
    <p role="alert" className={`${className} flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px] text-yellow`}>
      Oya Browser didn’t open, so it may not be installed. Install it first, then click Connect again.
      <a
        href="/downloads"
        target="_blank"
        rel="noreferrer"
        className="inline-flex items-center gap-1 font-medium underline"
      >
        <Download className="h-3.5 w-3.5" /> Download Oya Browser
      </a>
    </p>
  );
}
