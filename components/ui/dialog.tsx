import { ComponentChildren } from "preact";
import { JSX } from "preact/jsx-runtime";
import { createPortal } from "preact/compat";

interface DialogProps {
  open: boolean;
  onOpenChange?: (open: boolean) => void;
  children: ComponentChildren;
}

export function Dialog({ open, onOpenChange, children }: DialogProps) {
  console.log("Dialog render", { open });
  if (!open) return null;

  const handleBackdropClick = (e: JSX.TargetedMouseEvent<HTMLDivElement>) => {
    if (e.target === e.currentTarget) {
      onOpenChange?.(false);
    }
  };

  // Use createPortal to render dialog at document.body level, escaping any parent containers
  if (typeof document === "undefined") return null;

  return createPortal(
    <div
      className="fixed inset-0 z-[9999] flex items-center justify-center bg-black bg-opacity-50"
      onClick={handleBackdropClick}
      style={{
        zIndex: 9999,
        position: "fixed",
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
      }}
    >
      {children}
    </div>,
    document.body,
  );
}

type DivProps = JSX.HTMLAttributes<HTMLDivElement>;

interface DialogContentProps extends Omit<DivProps, "className" | "children"> {
  children: ComponentChildren;
  className?: string;
  onClose?: () => void;
}

export function DialogContent(
  { children, className = "", onClose: _onClose, ...rest }: DialogContentProps,
) {
  return (
    <div
      {...rest}
      className={`bg-white rounded-lg shadow-lg max-w-md w-full mx-4 ${className}`}
    >
      {children}
    </div>
  );
}

interface DialogHeaderProps {
  children: ComponentChildren;
  className?: string;
}

export function DialogHeader({ children, className = "" }: DialogHeaderProps) {
  return (
    <div className={`px-6 py-4 border-b ${className}`}>
      {children}
    </div>
  );
}

interface DialogTitleProps {
  children: ComponentChildren;
  className?: string;
}

export function DialogTitle({ children, className = "" }: DialogTitleProps) {
  return (
    <h2 className={`text-lg font-semibold ${className}`}>
      {children}
    </h2>
  );
}

interface DialogDescriptionProps {
  children: ComponentChildren;
  className?: string;
}

export function DialogDescription({ children, className = "" }: DialogDescriptionProps) {
  return (
    <p className={`text-sm text-gray-600 mt-1 ${className}`}>
      {children}
    </p>
  );
}

interface DialogFooterProps {
  children: ComponentChildren;
  className?: string;
}

export function DialogFooter({ children, className = "" }: DialogFooterProps) {
  return (
    <div className={`px-6 py-4 border-t bg-gray-50 flex justify-end space-x-2 ${className}`}>
      {children}
    </div>
  );
}
