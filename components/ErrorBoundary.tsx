/**
 * Error boundary component for graceful error handling
 */

import { Component, ComponentChildren } from "preact";
import { signal } from "@preact/signals";

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  errorInfo: string | null;
}

interface ErrorBoundaryProps {
  children: ComponentChildren;
  fallback?: (error: Error, retry: () => void) => ComponentChildren;
  onError?: (error: Error, errorInfo: string) => void;
  showDetails?: boolean;
}

// Global error tracking
export const globalErrorState = signal<{
  errors: Array<{ id: string; error: Error; timestamp: number; component?: string }>;
  count: number;
}>({
  errors: [],
  count: 0
});

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  private errorId: string;

  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null
    };
    this.errorId = crypto.randomUUID();
  }

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return {
      hasError: true,
      error
    };
  }

  componentDidCatch(error: Error, errorInfo: any) {
    const errorDetails = errorInfo?.componentStack || error.stack || 'No stack trace available';
    
    this.setState({
      errorInfo: errorDetails
    });

    // Track error globally
    const errorEntry = {
      id: this.errorId,
      error,
      timestamp: Date.now(),
      component: this.constructor.name
    };

    globalErrorState.value = {
      errors: [...globalErrorState.value.errors.slice(-9), errorEntry], // Keep last 10 errors
      count: globalErrorState.value.count + 1
    };

    // Call custom error handler
    if (this.props.onError) {
      this.props.onError(error, errorDetails);
    }

    // Log error for debugging
    console.error('ErrorBoundary caught an error:', error);
    console.error('Error details:', errorDetails);
  }

  retry = () => {
    this.setState({
      hasError: false,
      error: null,
      errorInfo: null
    });
  };

  render() {
    if (this.state.hasError && this.state.error) {
      // Use custom fallback if provided
      if (this.props.fallback) {
        return this.props.fallback(this.state.error, this.retry);
      }

      // Default error UI
      return (
        <div class="bg-red-50 border border-red-200 rounded-lg p-6 m-4">
          <div class="flex items-start space-x-3">
            <div class="text-red-600 text-2xl">⚠️</div>
            <div class="flex-1">
              <h3 class="text-lg font-semibold text-red-800 mb-2">
                Something went wrong
              </h3>
              <p class="text-red-700 mb-4">
                An error occurred while rendering this component. This might be a temporary issue.
              </p>
              
              <div class="flex space-x-3 mb-4">
                <button
                  onClick={this.retry}
                  class="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors text-sm font-medium"
                >
                  Try Again
                </button>
                <button
                  onClick={() => window.location.reload()}
                  class="px-4 py-2 bg-gray-600 text-white rounded-lg hover:bg-gray-700 transition-colors text-sm font-medium"
                >
                  Reload Page
                </button>
              </div>

              {this.props.showDetails && (
                <details class="mt-4">
                  <summary class="cursor-pointer text-sm font-medium text-red-800 hover:text-red-900">
                    Technical Details
                  </summary>
                  <div class="mt-2 p-3 bg-red-100 rounded border text-xs font-mono text-red-800 overflow-auto max-h-40">
                    <div class="mb-2">
                      <strong>Error:</strong> {this.state.error.message}
                    </div>
                    {this.state.errorInfo && (
                      <div>
                        <strong>Stack Trace:</strong>
                        <pre class="whitespace-pre-wrap mt-1">{this.state.errorInfo}</pre>
                      </div>
                    )}
                  </div>
                </details>
              )}
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

// Higher-order component for easy wrapping
export function withErrorBoundary<P extends object>(
  WrappedComponent: (props: P) => ComponentChildren,
  errorBoundaryProps?: Omit<ErrorBoundaryProps, 'children'>
) {
  return function WithErrorBoundaryComponent(props: P) {
    return (
      <ErrorBoundary {...errorBoundaryProps}>
        <WrappedComponent {...props} />
      </ErrorBoundary>
    );
  };
}

// Error notification component for global error display
export function ErrorNotifications() {
  const errors = globalErrorState.value.errors;
  const recentErrors = errors.filter(e => Date.now() - e.timestamp < 10000); // Last 10 seconds

  if (recentErrors.length === 0) {
    return null;
  }

  return (
    <div class="fixed top-4 right-4 z-50 space-y-2">
      {recentErrors.map((errorEntry) => (
        <div
          key={errorEntry.id}
          class="bg-red-100 border border-red-300 rounded-lg p-3 shadow-lg max-w-sm animate-slide-in"
        >
          <div class="flex items-start justify-between">
            <div class="flex-1">
              <div class="text-sm font-medium text-red-800">
                Component Error
              </div>
              <div class="text-xs text-red-600 mt-1">
                {errorEntry.error.message}
              </div>
            </div>
            <button
              onClick={() => {
                globalErrorState.value = {
                  ...globalErrorState.value,
                  errors: globalErrorState.value.errors.filter(e => e.id !== errorEntry.id)
                };
              }}
              class="text-red-400 hover:text-red-600 ml-2"
            >
              ✕
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}