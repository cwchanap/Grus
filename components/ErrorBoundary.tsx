import { Component, ComponentChildren } from "preact";
import { TouchButton } from "./MobileOptimized.tsx";
import process from "node:process";

interface ErrorBoundaryState {
  hasError: boolean;
  error?: Error;
  errorInfo?: any;
}

interface ErrorBoundaryProps {
  children: ComponentChildren;
  fallback?: (error: Error, retry: () => void) => ComponentChildren;
  onError?: (error: Error, errorInfo: any) => void;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static override getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return {
      hasError: true,
      error,
    };
  }

  override componentDidCatch(error: Error, errorInfo: any) {
    console.error("ErrorBoundary caught an error:", error, errorInfo);

    this.setState({
      error,
      errorInfo,
    });

    // Call the onError callback if provided
    if (this.props.onError) {
      this.props.onError(error, errorInfo);
    }
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: undefined, errorInfo: undefined });
  };

  render() {
    if (this.state.hasError) {
      // Use custom fallback if provided
      if (this.props.fallback && this.state.error) {
        return this.props.fallback(this.state.error, this.handleRetry);
      }

      // Default mobile-friendly error UI
      return (
        <div class="error-boundary bg-red-50 border border-red-200 rounded-lg p-4 sm:p-6 text-center">
          <div class="text-red-600 text-4xl sm:text-6xl mb-4">⚠️</div>
          <h2 class="text-lg sm:text-xl font-bold text-red-800 mb-2">
            Something went wrong
          </h2>
          <p class="text-sm sm:text-base text-red-700 mb-4 max-w-md mx-auto">
            We encountered an unexpected error. Please try refreshing the page or contact support if
            the problem persists.
          </p>

          {/* Error details (only in development) */}
          {process.env.NODE_ENV === "development" && this.state.error && (
            <details class="text-left bg-red-100 border border-red-300 rounded p-3 mb-4 text-xs sm:text-sm">
              <summary class="cursor-pointer font-medium text-red-800 mb-2">
                Error Details (Development)
              </summary>
              <pre class="whitespace-pre-wrap text-red-700 overflow-auto">
                {this.state.error.toString()}
                {this.state.errorInfo?.componentStack}
              </pre>
            </details>
          )}

          <div class="flex flex-col sm:flex-row gap-3 justify-center">
            <TouchButton
              onClick={this.handleRetry}
              variant="primary"
              size="md"
            >
              Try Again
            </TouchButton>
            <TouchButton
              onClick={() => globalThis.location.reload()}
              variant="secondary"
              size="md"
            >
              Refresh Page
            </TouchButton>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

// Hook version for functional components
export function useErrorHandler() {
  const handleError = (error: Error, errorInfo?: any) => {
    console.error("Error caught by error handler:", error, errorInfo);

    // In a real app, you might want to send this to an error reporting service
    // like Sentry, LogRocket, etc.
  };

  return handleError;
}

// Higher-order component for wrapping components with error boundary
export function withErrorBoundary<P extends object>(
  WrappedComponent: (props: P) => ComponentChildren,
  fallback?: (error: Error, retry: () => void) => ComponentChildren,
) {
  return function WithErrorBoundaryComponent(props: P) {
    const element = WrappedComponent(props);
    return (
      <ErrorBoundary fallback={fallback}>
        {element}
      </ErrorBoundary>
    );
  };
}

// Mobile-specific error fallback
export function MobileErrorFallback(_error: Error, retry: () => void) {
  return (
    <div class="mobile-error-fallback bg-red-50 border border-red-200 rounded-lg p-3 sm:p-4 text-center">
      <div class="text-red-600 text-2xl sm:text-4xl mb-2">⚠️</div>
      <h3 class="text-sm sm:text-base font-bold text-red-800 mb-2">
        Oops! Something went wrong
      </h3>
      <p class="text-xs sm:text-sm text-red-700 mb-3">
        Please try again or refresh the page.
      </p>
      <div class="flex gap-2">
        <TouchButton
          onClick={retry}
          variant="primary"
          size="sm"
          className="flex-1"
        >
          Retry
        </TouchButton>
        <TouchButton
          onClick={() => globalThis.location.reload()}
          variant="secondary"
          size="sm"
          className="flex-1"
        >
          Refresh
        </TouchButton>
      </div>
    </div>
  );
}
