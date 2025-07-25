import { useEffect, useState } from "preact/hooks";

interface ConnectionStatusProps {
  size?: 'sm' | 'md' | 'lg';
  showText?: boolean;
  className?: string;
}

export default function ConnectionStatus({ 
  size = 'md', 
  showText = true, 
  className = "" 
}: ConnectionStatusProps) {
  const [status, setStatus] = useState<'online' | 'offline' | 'slow'>('online');
  const [lastUpdate, setLastUpdate] = useState<number>(Date.now());

  useEffect(() => {
    const updateConnectionStatus = () => {
      if (!navigator.onLine) {
        setStatus('offline');
      } else {
        // Check connection speed/quality
        const connection = (navigator as any).connection;
        if (connection) {
          const { effectiveType, downlink } = connection;
          if (effectiveType === 'slow-2g' || effectiveType === '2g' || downlink < 0.5) {
            setStatus('slow');
          } else {
            setStatus('online');
          }
        } else {
          setStatus('online');
        }
      }
      setLastUpdate(Date.now());
    };

    // Initial check
    updateConnectionStatus();

    // Listen for online/offline events
    window.addEventListener('online', updateConnectionStatus);
    window.addEventListener('offline', updateConnectionStatus);

    // Listen for connection changes (if supported)
    if ((navigator as any).connection) {
      (navigator as any).connection.addEventListener('change', updateConnectionStatus);
    }

    // Periodic check
    const interval = setInterval(updateConnectionStatus, 30000); // Check every 30 seconds

    return () => {
      window.removeEventListener('online', updateConnectionStatus);
      window.removeEventListener('offline', updateConnectionStatus);
      if ((navigator as any).connection) {
        (navigator as any).connection.removeEventListener('change', updateConnectionStatus);
      }
      clearInterval(interval);
    };
  }, []);

  const getStatusConfig = () => {
    switch (status) {
      case 'online':
        return {
          color: 'bg-green-500',
          text: 'Online',
          icon: '🟢',
          pulse: false
        };
      case 'slow':
        return {
          color: 'bg-yellow-500',
          text: 'Slow',
          icon: '🟡',
          pulse: true
        };
      case 'offline':
        return {
          color: 'bg-red-500',
          text: 'Offline',
          icon: '🔴',
          pulse: true
        };
    }
  };

  const getSizeClasses = () => {
    switch (size) {
      case 'sm':
        return {
          dot: 'w-2 h-2',
          text: 'text-xs',
          container: 'gap-1'
        };
      case 'md':
        return {
          dot: 'w-3 h-3',
          text: 'text-sm',
          container: 'gap-2'
        };
      case 'lg':
        return {
          dot: 'w-4 h-4',
          text: 'text-base',
          container: 'gap-2'
        };
    }
  };

  const config = getStatusConfig();
  const sizeClasses = getSizeClasses();

  return (
    <div class={`flex items-center ${sizeClasses.container} ${className}`}>
      <div 
        class={`
          ${sizeClasses.dot} 
          ${config.color} 
          rounded-full 
          ${config.pulse ? 'animate-pulse' : ''}
        `}
        title={`Connection: ${config.text}`}
      />
      {showText && (
        <span class={`${sizeClasses.text} text-gray-600 font-medium`}>
          {config.text}
        </span>
      )}
    </div>
  );
}

// Hook for connection status
export function useConnectionStatus() {
  const [status, setStatus] = useState<'online' | 'offline' | 'slow'>('online');
  const [isOnline, setIsOnline] = useState(navigator.onLine);

  useEffect(() => {
    const updateStatus = () => {
      const online = navigator.onLine;
      setIsOnline(online);
      
      if (!online) {
        setStatus('offline');
        return;
      }

      // Check connection quality if available
      const connection = (navigator as any).connection;
      if (connection) {
        const { effectiveType, downlink } = connection;
        if (effectiveType === 'slow-2g' || effectiveType === '2g' || downlink < 0.5) {
          setStatus('slow');
        } else {
          setStatus('online');
        }
      } else {
        setStatus('online');
      }
    };

    updateStatus();

    window.addEventListener('online', updateStatus);
    window.addEventListener('offline', updateStatus);

    if ((navigator as any).connection) {
      (navigator as any).connection.addEventListener('change', updateStatus);
    }

    return () => {
      window.removeEventListener('online', updateStatus);
      window.removeEventListener('offline', updateStatus);
      if ((navigator as any).connection) {
        (navigator as any).connection.removeEventListener('change', updateStatus);
      }
    };
  }, []);

  return { status, isOnline };
}