import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef } from "preact/compat";
import { cn } from "../../lib/utils.ts";

const labelVariants = cva(
  "text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70",
);

// deno-lint-ignore no-empty-interface
export interface LabelProps
  extends preact.JSX.HTMLAttributes<HTMLLabelElement>,
    VariantProps<typeof labelVariants> {}

const Label = forwardRef<
  HTMLLabelElement,
  LabelProps
>(({ className, ...props }, ref) => (
  <label
    ref={ref}
    className={cn(labelVariants(), className)}
    {...props}
  />
));
Label.displayName = "Label";

export { Label };
