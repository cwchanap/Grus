import { useEffect, useState } from "preact/hooks";

interface ToastMessage {
  message: string;
  type: 'success' | 'error' | 'info';
  id: string;
}

export default function Toast() {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  useEffect(() => {
    const handleShowToast = (event: Event) => {
      const customEvent = event as CustomEvent;
      const { message, type = 'info' } = customEvent.detail;

      const newToast: ToastMessage = {
        message,
        type,
        id: Date.now().toString(),
      };

      setToasts((prev) => [...prev, newToast]);

      // Auto-remove toast after 3 seconds
      setTimeout(() => {
        setToasts((prev) => prev.filter((toast) => toast.id !== newToast.id));
      }, 3000);
    };

    globalThis.addEventListener('showToast', handleShowToast);

    return () => {
      globalThis.removeEventListener('showToast', handleShowToast);
    };
  }, []);

  const getToastStyles = (type: string) => {
    switch (type) {
      case 'success':
        return 'bg-green-500 text-white';
      case 'error':
        return 'bg-red-500 text-white';
      case 'info':
      default:
        return 'bg-blue-500 text-white';
    }
  };

  return (
    <div class="fixed top-4 right-4 z-50 space-y-2">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          class={`px-4 py-2 rounded-lg shadow-lg animate-slide-in ${getToastStyles(toast.type)}`}
        >
          {toast.message}
        </div>
      ))}
    </div>
  );
}