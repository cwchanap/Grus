import { ComponentChildren } from "preact";
import { useEffect, useState } from "preact/hooks";

interface MobileOptimizedProps {
  children: ComponentChildren;
  className?: string;
}

// Hook to detect mobile device
export function useIsMobile() {
  const [isMobile, setIsMobile] = useState(false);
  const [isTouch, setIsTouch] = useState(false);

  useEffect(() => {
    const checkMobile = () => {
      const userAgent = navigator.userAgent.toLowerCase();
      const mobileKeywords = [
        "mobile",
        "android",
        "iphone",
        "ipad",
        "ipod",
        "blackberry",
        "windows phone",
      ];
      const isMobileDevice = mobileKeywords.some((keyword) => userAgent.includes(keyword));

      setIsMobile(isMobileDevice || globalThis.innerWidth < 768);
      setIsTouch("ontouchstart" in window || navigator.maxTouchPoints > 0);
    };

    checkMobile();
    globalThis.addEventListener("resize", checkMobile);

    return () => globalThis.removeEventListener("resize", checkMobile);
  }, []);

  return { isMobile, isTouch };
}

// Hook for viewport dimensions
export function useViewport() {
  const [viewport, setViewport] = useState({
    width: typeof window !== "undefined" ? globalThis.innerWidth : 0,
    height: typeof window !== "undefined" ? globalThis.innerHeight : 0,
  });

  useEffect(() => {
    const handleResize = () => {
      setViewport({
        width: globalThis.innerWidth,
        height: globalThis.innerHeight,
      });
    };

    globalThis.addEventListener("resize", handleResize);
    return () => globalThis.removeEventListener("resize", handleResize);
  }, []);

  return viewport;
}

// Component for mobile-optimized containers
export function MobileContainer({ children, className = "" }: MobileOptimizedProps) {
  const { isMobile } = useIsMobile();

  return (
    <div
      class={`
      ${isMobile ? "px-2 py-2" : "px-4 py-4"}
      ${className}
    `}
    >
      {children}
    </div>
  );
}

// Component for touch-optimized buttons
interface TouchButtonProps {
  onClick: () => void;
  disabled?: boolean;
  variant?: "primary" | "secondary" | "danger";
  size?: "sm" | "md" | "lg";
  children: ComponentChildren;
  className?: string;
}

export function TouchButton({
  onClick,
  disabled = false,
  variant = "primary",
  size = "md",
  children,
  className = "",
}: TouchButtonProps) {
  const { isTouch } = useIsMobile();

  const baseClasses =
    "font-medium rounded-lg transition-all duration-200 touch-manipulation no-tap-highlight no-select";

  const variantClasses = {
    primary: disabled
      ? "bg-gray-300 text-gray-500 cursor-not-allowed"
      : "bg-blue-600 text-white hover:bg-blue-700 active:bg-blue-800",
    secondary: disabled
      ? "bg-gray-200 text-gray-400 cursor-not-allowed"
      : "bg-gray-100 text-gray-700 hover:bg-gray-200 active:bg-gray-300",
    danger: disabled
      ? "bg-gray-300 text-gray-500 cursor-not-allowed"
      : "bg-red-600 text-white hover:bg-red-700 active:bg-red-800",
  };

  const sizeClasses = {
    sm: isTouch ? "px-3 py-2 text-sm min-h-[44px]" : "px-2 py-1 text-sm",
    md: isTouch ? "px-4 py-3 text-base min-h-[44px]" : "px-3 py-2 text-base",
    lg: isTouch ? "px-6 py-4 text-lg min-h-[48px]" : "px-4 py-3 text-lg",
  };

  const activeClasses = isTouch && !disabled ? "active:scale-95" : "";

  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      class={`${baseClasses} ${variantClasses[variant]} ${
        sizeClasses[size]
      } ${activeClasses} ${className}`}
    >
      {children}
    </button>
  );
}

// Component for mobile-optimized input fields
interface TouchInputProps {
  type?: string;
  value: string;
  onInput: (e: Event) => void;
  placeholder?: string;
  disabled?: boolean;
  maxLength?: number;
  className?: string;
}

export function TouchInput({
  type = "text",
  value,
  onInput,
  placeholder,
  disabled = false,
  maxLength,
  className = "",
}: TouchInputProps) {
  const { isTouch } = useIsMobile();

  const baseClasses =
    "border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors touch-manipulation";
  const sizeClasses = isTouch ? "px-4 py-3 text-base min-h-[44px]" : "px-3 py-2 text-sm";

  return (
    <input
      type={type}
      value={value}
      onInput={onInput}
      placeholder={placeholder}
      disabled={disabled}
      maxLength={maxLength}
      class={`${baseClasses} ${sizeClasses} ${className}`}
      style={{ fontSize: isTouch ? "16px" : undefined }} // Prevent zoom on iOS
    />
  );
}

// Hook for safe area insets (for devices with notches)
export function useSafeArea() {
  const [safeArea, setSafeArea] = useState({
    top: 0,
    bottom: 0,
    left: 0,
    right: 0,
  });

  useEffect(() => {
    const updateSafeArea = () => {
      const computedStyle = getComputedStyle(document.documentElement);
      setSafeArea({
        top: parseInt(computedStyle.getPropertyValue("--safe-area-inset-top") || "0"),
        bottom: parseInt(computedStyle.getPropertyValue("--safe-area-inset-bottom") || "0"),
        left: parseInt(computedStyle.getPropertyValue("--safe-area-inset-left") || "0"),
        right: parseInt(computedStyle.getPropertyValue("--safe-area-inset-right") || "0"),
      });
    };

    updateSafeArea();
    globalThis.addEventListener("resize", updateSafeArea);

    return () => globalThis.removeEventListener("resize", updateSafeArea);
  }, []);

  return safeArea;
}

// Component for responsive grid layouts
interface ResponsiveGridProps {
  children: ComponentChildren;
  cols?: {
    xs?: number;
    sm?: number;
    md?: number;
    lg?: number;
    xl?: number;
  };
  gap?: number;
  className?: string;
}

export function ResponsiveGrid({
  children,
  cols = { xs: 1, sm: 2, md: 3, lg: 4 },
  gap = 4,
  className = "",
}: ResponsiveGridProps) {
  const gridClasses = [
    `grid gap-${gap}`,
    cols.xs && `grid-cols-${cols.xs}`,
    cols.sm && `sm:grid-cols-${cols.sm}`,
    cols.md && `md:grid-cols-${cols.md}`,
    cols.lg && `lg:grid-cols-${cols.lg}`,
    cols.xl && `xl:grid-cols-${cols.xl}`,
    className,
  ].filter(Boolean).join(" ");

  return (
    <div class={gridClasses}>
      {children}
    </div>
  );
}
