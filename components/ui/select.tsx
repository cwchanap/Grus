import { ComponentChildren } from "preact";
import { JSX } from "preact/jsx-runtime";

interface SelectProps {
  value: string;
  onChange: (e: JSX.TargetedEvent<HTMLSelectElement>) => void;
  children: ComponentChildren;
  className?: string;
  disabled?: boolean;
}

export function Select(
  { value, onChange, children, className = "", disabled = false }: SelectProps,
) {
  return (
    <select
      value={value}
      onChange={onChange}
      disabled={disabled}
      className={`flex h-10 w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 ${className}`}
    >
      {children}
    </select>
  );
}

interface SelectItemProps {
  value: string;
  children: ComponentChildren;
  disabled?: boolean;
}

export function SelectItem({ value, children, disabled = false }: SelectItemProps) {
  return (
    <option value={value} disabled={disabled}>
      {children}
    </option>
  );
}
