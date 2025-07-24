/**
 * User-friendly error messages and recovery suggestions
 */

export interface ErrorInfo {
  title: string;
  message: string;
  suggestion: string;
  action?: {
    label: string;
    handler: () => void;
  };
}

export function getErrorInfo(error: Error | string): ErrorInfo {
  const errorMessage = typeof error === 'string' ? error : error.message;
  const errorType = typeof error === 'string' ? error : error.name;

  // Network-related errors
  if (errorMessage.includes('fetch') || errorMessage.includes('network') || errorMessage.includes('connection')) {
    return {
      title: 'Connection Problem',
      message: 'Unable to connect to the game server.',
      suggestion: 'Check your internet connection and try again.',
      action: {
        label: 'Retry Connection',
        handler: () => window.location.reload()
      }
    };
  }

  // WebSocket errors
  if (errorMessage.includes('websocket') || errorMessage.includes('WebSocket')) {
    return {
      title: 'Real-time Connection Lost',
      message: 'The real-time connection to the game was interrupted.',
      suggestion: 'The game will attempt to reconnect automatically.',
      action: {
        label: 'Force Reconnect',
        handler: () => window.location.reload()
      }
    };
  }

  // Room-related errors
  if (errorMessage.includes('room') || errorMessage.includes('Room')) {
    return {
      title: 'Room Error',
      message: 'There was a problem with the game room.',
      suggestion: 'Try returning to the lobby and joining again.',
      action: {
        label: 'Back to Lobby',
        handler: () => window.location.href = '/'
      }
    };
  }

  // Drawing-related errors
  if (errorMessage.includes('draw') || errorMessage.includes('canvas') || errorMessage.includes('Pixi')) {
    return {
      title: 'Drawing Error',
      message: 'There was a problem with the drawing system.',
      suggestion: 'Try refreshing the page to reset the drawing board.',
      action: {
        label: 'Refresh Page',
        handler: () => window.location.reload()
      }
    };
  }

  // Permission errors
  if (errorMessage.includes('permission') || errorMessage.includes('unauthorized')) {
    return {
      title: 'Permission Denied',
      message: 'You don\'t have permission to perform this action.',
      suggestion: 'Make sure you\'re the room host or try rejoining the room.',
      action: {
        label: 'Rejoin Room',
        handler: () => window.location.reload()
      }
    };
  }

  // Rate limiting errors
  if (errorMessage.includes('rate limit') || errorMessage.includes('too many')) {
    return {
      title: 'Too Many Requests',
      message: 'You\'re sending messages or actions too quickly.',
      suggestion: 'Please wait a moment before trying again.',
      action: {
        label: 'Wait and Retry',
        handler: () => setTimeout(() => window.location.reload(), 3000)
      }
    };
  }

  // Database errors
  if (errorMessage.includes('database') || errorMessage.includes('DB')) {
    return {
      title: 'Server Error',
      message: 'The game server is experiencing technical difficulties.',
      suggestion: 'Please try again in a few moments.',
      action: {
        label: 'Try Again',
        handler: () => window.location.reload()
      }
    };
  }

  // Generic errors
  return {
    title: 'Something Went Wrong',
    message: errorMessage || 'An unexpected error occurred.',
    suggestion: 'Try refreshing the page or returning to the lobby.',
    action: {
      label: 'Refresh Page',
      handler: () => window.location.reload()
    }
  };
}

// Error recovery strategies
export const ErrorRecoveryStrategies = {
  // Retry with exponential backoff
  retryWithBackoff: async (
    operation: () => Promise<any>,
    maxAttempts = 3,
    baseDelay = 1000
  ): Promise<any> => {
    for (let attempt = 1; attempt <= maxAttempts; attempt++) {
      try {
        return await operation();
      } catch (error) {
        if (attempt === maxAttempts) {
          throw error;
        }
        
        const delay = baseDelay * Math.pow(2, attempt - 1);
        await new Promise(resolve => setTimeout(resolve, delay));
      }
    }
  },

  // Graceful degradation
  withFallback: async <T>(
    primary: () => Promise<T>,
    fallback: () => T
  ): Promise<T> => {
    try {
      return await primary();
    } catch (error) {
      console.warn('Primary operation failed, using fallback:', error);
      return fallback();
    }
  },

  // Circuit breaker pattern
  createCircuitBreaker: (
    operation: () => Promise<any>,
    failureThreshold = 5,
    resetTimeout = 60000
  ) => {
    let failures = 0;
    let lastFailureTime = 0;
    let state: 'closed' | 'open' | 'half-open' = 'closed';

    return async () => {
      const now = Date.now();

      // Reset if enough time has passed
      if (state === 'open' && now - lastFailureTime > resetTimeout) {
        state = 'half-open';
        failures = 0;
      }

      // Reject if circuit is open
      if (state === 'open') {
        throw new Error('Circuit breaker is open');
      }

      try {
        const result = await operation();
        
        // Success - reset circuit
        if (state === 'half-open') {
          state = 'closed';
        }
        failures = 0;
        
        return result;
      } catch (error) {
        failures++;
        lastFailureTime = now;

        // Open circuit if threshold reached
        if (failures >= failureThreshold) {
          state = 'open';
        }

        throw error;
      }
    };
  }
};