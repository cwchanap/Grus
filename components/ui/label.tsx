import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef } from "preact/compat";
import { cn } from "../../lib/utils.ts";

const labelVariants = cva(
  "text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70",
);

const Label = forwardRef<
  HTMLLabelElement,
  & preact.JSX.HTMLAttributes<HTMLLabelElement>
  & VariantProps<typeof labelVariants>
>(({ className, ...props }, ref) => (
  <label
    ref={ref}
    className={cn(labelVariants(), className)}
    {...props}
  />
));
Label.displayName = "Label";

export { Label };
