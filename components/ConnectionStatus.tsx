/**
 * Connection status indicator component
 */

import { useComputed } from "@preact/signals";
import { connectionState } from "../lib/websocket/connection-manager.ts";

interface ConnectionStatusProps {
  showText?: boolean;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

export default function ConnectionStatus({ 
  showText = true, 
  size = 'md',
  className = '' 
}: ConnectionStatusProps) {
  const state = useComputed(() => connectionState.value);

  const getStatusConfig = () => {
    const status = state.value.status;
    const isOnline = state.value.isOnline;

    if (!isOnline) {
      return {
        color: 'bg-gray-500',
        text: 'Offline',
        icon: '📡',
        description: 'No internet connection'
      };
    }

    switch (status) {
      case 'connected':
        return {
          color: 'bg-green-500',
          text: 'Connected',
          icon: '🟢',
          description: 'Connected to game server'
        };
      case 'connecting':
        return {
          color: 'bg-yellow-500',
          text: 'Connecting...',
          icon: '🟡',
          description: 'Connecting to game server'
        };
      case 'reconnecting':
        return {
          color: 'bg-orange-500',
          text: `Reconnecting... (${state.value.reconnectAttempts})`,
          icon: '🔄',
          description: 'Attempting to reconnect'
        };
      case 'offline':
        return {
          color: 'bg-gray-500',
          text: 'Offline',
          icon: '📡',
          description: 'No internet connection'
        };
      case 'disconnected':
      default:
        return {
          color: 'bg-red-500',
          text: 'Disconnected',
          icon: '🔴',
          description: state.value.error || 'Disconnected from server'
        };
    }
  };

  const getSizeClasses = () => {
    switch (size) {
      case 'sm':
        return {
          dot: 'w-2 h-2',
          text: 'text-xs',
          container: 'space-x-1'
        };
      case 'lg':
        return {
          dot: 'w-4 h-4',
          text: 'text-base',
          container: 'space-x-3'
        };
      case 'md':
      default:
        return {
          dot: 'w-3 h-3',
          text: 'text-sm',
          container: 'space-x-2'
        };
    }
  };

  const statusConfig = getStatusConfig();
  const sizeClasses = getSizeClasses();

  return (
    <div 
      class={`flex items-center ${sizeClasses.container} ${className}`}
      title={statusConfig.description}
    >
      <div class={`${sizeClasses.dot} rounded-full ${statusConfig.color} ${
        state.value.status === 'connecting' || state.value.status === 'reconnecting' 
          ? 'animate-pulse' 
          : ''
      }`}></div>
      
      {showText && (
        <span class={`${sizeClasses.text} text-gray-600 font-medium`}>
          {statusConfig.text}
        </span>
      )}
      
      {state.value.error && size === 'lg' && (
        <span class="text-xs text-red-600 ml-2">
          {state.value.error}
        </span>
      )}
    </div>
  );
}