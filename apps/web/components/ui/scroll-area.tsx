import { ComponentChildren } from "preact";
import { cn } from "../../lib/utils.ts";

interface ScrollAreaProps {
  className?: string;
  children: ComponentChildren;
}

export function ScrollArea({ className, children }: ScrollAreaProps) {
  return (
    <div className={cn("overflow-auto", className)}>
      {children}
    </div>
  );
}
