import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef } from "preact/compat";
import { type JSX } from "preact";
import { cn } from "../../lib/utils.ts";

const labelVariants = cva(
  "text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70",
);

const Label = forwardRef<
  HTMLLabelElement,
  & JSX.HTMLAttributes<HTMLLabelElement>
  & VariantProps<typeof labelVariants>
  & { [key: string]: unknown }
>(({ className, ...props }, ref) => (
  <label
    ref={ref}
    className={cn(labelVariants(), className)}
    {...props}
  />
));
Label.displayName = "Label";

export { Label };
