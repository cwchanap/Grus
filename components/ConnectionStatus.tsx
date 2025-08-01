import { useEffect, useState } from "preact/hooks";
import { Badge } from "./ui/badge.tsx";
import { cn } from "../lib/utils.ts";

interface ConnectionStatusProps {
  size?: "sm" | "md" | "lg";
  showText?: boolean;
  className?: string;
}

export default function ConnectionStatus({
  size = "md",
  showText = true,
  className = "",
}: ConnectionStatusProps) {
  const [status, setStatus] = useState<"online" | "offline" | "slow">("online");
  const [lastUpdate, setLastUpdate] = useState<number>(Date.now());

  useEffect(() => {
    const updateConnectionStatus = () => {
      if (!navigator.onLine) {
        setStatus("offline");
      } else {
        // Check connection speed/quality
        const connection = (navigator as any).connection;
        if (connection) {
          const { effectiveType, downlink } = connection;
          if (effectiveType === "slow-2g" || effectiveType === "2g" || downlink < 0.5) {
            setStatus("slow");
          } else {
            setStatus("online");
          }
        } else {
          setStatus("online");
        }
      }
      setLastUpdate(Date.now());
    };

    // Initial check
    updateConnectionStatus();

    // Listen for online/offline events
    globalThis.addEventListener("online", updateConnectionStatus);
    globalThis.addEventListener("offline", updateConnectionStatus);

    // Listen for connection changes (if supported)
    if ((navigator as any).connection) {
      (navigator as any).connection.addEventListener("change", updateConnectionStatus);
    }

    // Periodic check
    const interval = setInterval(updateConnectionStatus, 30000); // Check every 30 seconds

    return () => {
      globalThis.removeEventListener("online", updateConnectionStatus);
      globalThis.removeEventListener("offline", updateConnectionStatus);
      if ((navigator as any).connection) {
        (navigator as any).connection.removeEventListener("change", updateConnectionStatus);
      }
      clearInterval(interval);
    };
  }, []);

  const getStatusConfig = () => {
    switch (status) {
      case "online":
        return {
          variant: "default" as const,
          text: "Online",
          dotColor: "bg-green-500",
          pulse: false,
        };
      case "slow":
        return {
          variant: "secondary" as const,
          text: "Slow",
          dotColor: "bg-yellow-500",
          pulse: true,
        };
      case "offline":
        return {
          variant: "destructive" as const,
          text: "Offline",
          dotColor: "bg-red-500",
          pulse: true,
        };
    }
  };

  const getSizeClasses = () => {
    switch (size) {
      case "sm":
        return {
          dot: "w-2 h-2",
          container: "gap-1",
        };
      case "md":
        return {
          dot: "w-3 h-3",
          container: "gap-2",
        };
      case "lg":
        return {
          dot: "w-4 h-4",
          container: "gap-2",
        };
    }
  };

  const config = getStatusConfig();
  const sizeClasses = getSizeClasses();

  if (showText) {
    return (
      <Badge
        variant={config.variant}
        className={cn("flex items-center", sizeClasses.container, className)}
      >
        <div
          className={cn(
            sizeClasses.dot,
            config.dotColor,
            "rounded-full mr-1",
            config.pulse && "animate-pulse",
          )}
          title={`Connection: ${config.text}`}
        />
        {config.text}
      </Badge>
    );
  }

  return (
    <div
      className={cn(
        sizeClasses.dot,
        config.dotColor,
        "rounded-full",
        config.pulse && "animate-pulse",
        className,
      )}
      title={`Connection: ${config.text}`}
    />
  );
}

// Hook for connection status
export function useConnectionStatus() {
  const [status, setStatus] = useState<"online" | "offline" | "slow">("online");
  const [isOnline, setIsOnline] = useState(navigator.onLine);

  useEffect(() => {
    const updateStatus = () => {
      const online = navigator.onLine;
      setIsOnline(online);

      if (!online) {
        setStatus("offline");
        return;
      }

      // Check connection quality if available
      const connection = (navigator as any).connection;
      if (connection) {
        const { effectiveType, downlink } = connection;
        if (effectiveType === "slow-2g" || effectiveType === "2g" || downlink < 0.5) {
          setStatus("slow");
        } else {
          setStatus("online");
        }
      } else {
        setStatus("online");
      }
    };

    updateStatus();

    globalThis.addEventListener("online", updateStatus);
    globalThis.addEventListener("offline", updateStatus);

    if ((navigator as any).connection) {
      (navigator as any).connection.addEventListener("change", updateStatus);
    }

    return () => {
      globalThis.removeEventListener("online", updateStatus);
      globalThis.removeEventListener("offline", updateStatus);
      if ((navigator as any).connection) {
        (navigator as any).connection.removeEventListener("change", updateStatus);
      }
    };
  }, []);

  return { status, isOnline };
}
