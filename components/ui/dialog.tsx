import { ComponentChildren } from "preact";
import { JSX } from "preact/jsx-runtime";

interface DialogProps {
  open: boolean;
  onOpenChange?: (open: boolean) => void;
  children: ComponentChildren;
}

export function Dialog({ open, onOpenChange, children }: DialogProps) {
  if (!open) return null;

  const handleBackdropClick = (e: JSX.TargetedMouseEvent<HTMLDivElement>) => {
    if (e.target === e.currentTarget) {
      onOpenChange?.(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50"
      onClick={handleBackdropClick}
    >
      {children}
    </div>
  );
}

interface DialogContentProps {
  children: ComponentChildren;
  className?: string;
  onClose?: () => void;
}

export function DialogContent({ children, className = "", onClose }: DialogContentProps) {
  return (
    <div className={`bg-white rounded-lg shadow-lg max-w-md w-full mx-4 ${className}`}>
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
