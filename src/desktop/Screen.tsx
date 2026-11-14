import { useEffect, useRef, type ReactNode } from "react";
import { Icon } from "./editor/Icons";

/** A view other than the editor. It takes focus when it opens; Escape closes it, unless the key belongs to a field
 *  or to an open confirmation. */
export function Screen({ className, onClose, children }: { className: string; onClose(): void; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => { ref.current?.focus({ preventScroll: true }); }, []);
  return (
    <div ref={ref} tabIndex={-1} className={`screen ${className}`}
      onKeyDown={(e) => {
        if (e.key === "Escape" && !e.defaultPrevented && !(e.target as Element).closest("input, textarea, select, [role=group]")) onClose();
      }}>
      {children}
    </div>
  );
}

export function CloseButton({ label, onClose, className = "x-btn" }: { label: string; onClose(): void; className?: string }) {
  return (
    <button type="button" className={className} aria-label={label} title={label} onClick={onClose}>
      <span className="x-ring"><Icon name="close" size={18} /></span>
      <kbd aria-hidden="true">Esc</kbd>
    </button>
  );
}
