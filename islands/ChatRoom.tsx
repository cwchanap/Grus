import { useEffect, useState, useRef } from "preact/hooks";
import { useComputed } from "@preact/signals";
import type { ChatMessage } from "../types/game.ts";

// Extend ChatMessage type for offline support
interface ExtendedChatMessage extends ChatMessage {
  isOffline?: boolean;
}
import { WebSocketConnectionManager, connectionState } from "../lib/websocket/connection-manager.ts";
import { OfflineManager, offlineState } from "../lib/offline-manager.ts";
import { ErrorBoundary } from "../components/ErrorBoundary.tsx";
import ConnectionStatus from "../components/ConnectionStatus.tsx";

interface ChatRoomProps {
  roomId: string;
  playerId: string;
  playerName: string;
  currentWord?: string;
  isCurrentDrawer?: boolean;
}

function ChatRoomComponent({ 
  roomId, 
  playerId, 
  playerName, 
  currentWord, 
  isCurrentDrawer = false 
}: ChatRoomProps) {
  const [messages, setMessages] = useState<ExtendedChatMessage[]>([]);
  const [inputMessage, setInputMessage] = useState('');
  const [isTyping, setIsTyping] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const connectionManagerRef = useRef<WebSocketConnectionManager | null>(null);
  const offlineManagerRef = useRef<OfflineManager | null>(null);
  
  const connectionStatus = useComputed(() => connectionState.value);
  const offlineStatus = useComputed(() => offlineState.value);

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // Connection and offline management
  useEffect(() => {
    // Initialize connection manager
    connectionManagerRef.current = new WebSocketConnectionManager(
      roomId,
      playerId,
      playerName
    );

    // Initialize offline manager
    offlineManagerRef.current = new OfflineManager(roomId);

    // Set up message handlers
    connectionManagerRef.current.onMessage('chat-message', (message) => {
      const chatMessage: ChatMessage = message.data;
      setMessages(prev => [...prev, chatMessage]);
    });

    connectionManagerRef.current.onMessage('room-update', (message) => {
      if (message.data?.type === 'error') {
        console.error('Chat error:', message.data.message);
      }
    });

    return () => {
      connectionManagerRef.current?.destroy();
      offlineManagerRef.current?.destroy();
    };
  }, [roomId, playerId, playerName]);

  const sendMessage = (e: Event) => {
    e.preventDefault();
    
    const message = inputMessage.trim();
    if (!message || message.length > 200) return;
    
    const connectionManager = connectionManagerRef.current;
    const offlineManager = offlineManagerRef.current;
    
    if (!connectionManager) return;

    const chatMessage: ChatMessage = {
      id: crypto.randomUUID(),
      playerId,
      playerName,
      message,
      timestamp: Date.now(),
      isCorrect: false
    };

    // Try to send message
    const sent = connectionManager.sendMessage({
      type: 'chat',
      roomId,
      playerId,
      data: { text: message }
    });

    if (!sent && offlineManager) {
      // Queue message for offline mode
      offlineManager.queueChatMessage(chatMessage);
      
      // Add to local messages with offline indicator
      setMessages(prev => [...prev, { ...chatMessage, isOffline: true }]);
    }
    
    setInputMessage('');
    inputRef.current?.focus();
  };

  const handleInputChange = (e: Event) => {
    const value = (e.target as HTMLInputElement).value;
    setInputMessage(value);
    
    // Show typing indicator briefly
    setIsTyping(true);
    setTimeout(() => setIsTyping(false), 1000);
  };

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  };

  const isCorrectGuess = (message: ChatMessage) => {
    return message.isCorrect && message.playerId !== playerId;
  };

  const isOwnMessage = (message: ChatMessage) => {
    return message.playerId === playerId;
  };

  const shouldShowGuessHint = () => {
    return !isCurrentDrawer && currentWord && messages.length > 0;
  };

  return (
    <div class="flex flex-col h-full">
      {/* Header with connection status - Mobile responsive */}
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-base sm:text-lg font-semibold text-gray-800">Chat</h2>
        <div class="flex items-center space-x-2 sm:space-x-3">
          <ConnectionStatus size="sm" />
          {offlineStatus.value.isOffline && offlineStatus.value.pendingMessages.length > 0 && (
            <div class="text-xs text-orange-600 bg-orange-50 px-2 py-1 rounded">
              <span class="hidden sm:inline">{offlineStatus.value.pendingMessages.length} pending</span>
              <span class="sm:hidden">{offlineStatus.value.pendingMessages.length}</span>
            </div>
          )}
        </div>
      </div>

      {/* Messages area */}
      <div class="flex-1 bg-gray-50 rounded-lg p-3 mb-3 overflow-y-auto min-h-0">
        <div class="space-y-2">
          {messages.length === 0 ? (
            <div class="text-center text-gray-500 py-8">
              <div class="text-2xl mb-2">💬</div>
              <p class="text-sm">
                {isCurrentDrawer 
                  ? "Others will guess your drawing here!" 
                  : "Start guessing what's being drawn!"}
              </p>
            </div>
          ) : (
            messages.map((message) => (
              <div
                key={message.id}
                class={`flex flex-col space-y-1 ${
                  isOwnMessage(message) ? 'items-end' : 'items-start'
                }`}
              >
                <div
                  class={`max-w-xs sm:max-w-sm px-3 py-2 rounded-lg text-sm ${
                    isCorrectGuess(message)
                      ? 'bg-green-100 border border-green-300 text-green-800'
                      : isOwnMessage(message)
                      ? 'bg-blue-500 text-white'
                      : 'bg-white border border-gray-200 text-gray-800'
                  }`}
                >
                  {!isOwnMessage(message) && (
                    <div class="text-xs font-medium mb-1 opacity-75">
                      {message.playerName}
                      {isCorrectGuess(message) && ' 🎉'}
                    </div>
                  )}
                  <div class="break-words">
                    {message.message}
                  </div>
                  {isCorrectGuess(message) && (
                    <div class="text-xs mt-1 font-medium">
                      Correct guess!
                    </div>
                  )}
                </div>
                <div class="text-xs text-gray-400">
                  {formatTime(message.timestamp)}
                </div>
              </div>
            ))
          )}
          {isTyping && (
            <div class="flex items-center space-x-2 text-gray-500 text-sm">
              <div class="flex space-x-1">
                <div class="w-1 h-1 bg-gray-400 rounded-full animate-bounce"></div>
                <div class="w-1 h-1 bg-gray-400 rounded-full animate-bounce" style="animation-delay: 0.1s"></div>
                <div class="w-1 h-1 bg-gray-400 rounded-full animate-bounce" style="animation-delay: 0.2s"></div>
              </div>
              <span>Someone is typing...</span>
            </div>
          )}
          <div ref={messagesEndRef} />
        </div>
      </div>

      {/* Guess hint for non-drawers */}
      {shouldShowGuessHint() && (
        <div class="bg-blue-50 border border-blue-200 rounded-lg p-2 mb-3">
          <div class="text-xs text-blue-700 font-medium">
            💡 Tip: Type your guess and press Enter!
          </div>
        </div>
      )}

      {/* Message input - Mobile optimized */}
      <form onSubmit={sendMessage} class="flex space-x-2">
        <input
          ref={inputRef}
          type="text"
          value={inputMessage}
          onInput={handleInputChange}
          placeholder={
            isCurrentDrawer 
              ? "You're drawing! Others will guess..." 
              : "Type your guess here..."
          }
          maxLength={200}
          disabled={false} // Allow typing in offline mode
          class="flex-1 px-3 py-2 sm:py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-sm touch-manipulation"
        />
        <button
          type="submit"
          disabled={!inputMessage.trim() || inputMessage.length > 200}
          class="px-3 sm:px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium touch-manipulation"
        >
          <span class="hidden sm:inline">{offlineStatus.value.isOffline ? 'Queue' : 'Send'}</span>
          <span class="sm:hidden">→</span>
        </button>
      </form>

      {/* Character count */}
      <div class="text-xs text-gray-500 mt-1 text-right">
        {inputMessage.length}/200
      </div>
    </div>
  );
}

// Export with error boundary
export default function ChatRoom(props: ChatRoomProps) {
  return (
    <ErrorBoundary
      fallback={(error, retry) => (
        <div class="bg-red-50 border border-red-200 rounded-lg p-4">
          <div class="text-red-800 font-medium mb-2">Chat Error</div>
          <div class="text-red-600 text-sm mb-3">
            Failed to load chat: {error.message}
          </div>
          <button
            onClick={retry}
            class="px-3 py-1 bg-red-600 text-white rounded text-sm hover:bg-red-700"
          >
            Retry
          </button>
        </div>
      )}
    >
      <ChatRoomComponent {...props} />
    </ErrorBoundary>
  );
}