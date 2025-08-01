import { ComponentChildren } from "preact";
import { forwardRef } from "preact/compat";
import { cn } from "../../lib/utils.ts";

interface SelectProps extends preact.JSX.HTMLAttributes<HTMLSelectElement> {
  children: ComponentChildren;
}

const Select = forwardRef<HTMLSelectElement, SelectProps>(
  ({ className, children, ...props }, ref) => (
    <div className="relative">
      <select
        className={cn(
          "flex h-10 w-full items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 appearance-none",
          className,
        )}
        ref={ref}
        {...props}
      >
        {children}
      </select>
      <svg
        className="absolute right-3 top-3 h-4 w-4 opacity-50 pointer-events-none"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
      </svg>
    </div>
  ),
);
Select.displayName = "Select";

const SelectContent = ({ children }: { children: ComponentChildren }) => <>{children}</>;

const SelectItem = forwardRef<
  HTMLOptionElement,
  preact.JSX.HTMLAttributes<HTMLOptionElement> & { value: string }
>(({ className, children, ...props }, ref) => (
  <option
    ref={ref}
    className={cn(
      "relative flex w-full cursor-default select-none items-center rounded-sm py-1.5 pl-8 pr-2 text-sm outline-none focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
      className,
    )}
    {...props}
  >
    {children}
  </option>
));
SelectItem.displayName = "SelectItem";

const SelectValue = ({ placeholder }: { placeholder?: string }) => (
  <option value="" disabled selected hidden>
    {placeholder}
  </option>
);

export { Select, SelectContent, SelectItem, SelectValue };
