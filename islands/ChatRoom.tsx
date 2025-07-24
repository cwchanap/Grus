import { useEffect, useState, useRef } from "preact/hooks";
import { signal } from "@preact/signals";
import type { ChatMessage } from "../types/game.ts";

interface ChatRoomProps {
  roomId: string;
  playerId: string;
  playerName: string;
  currentWord?: string;
  isCurrentDrawer?: boolean;
}

// Global signal for WebSocket connection
const wsConnection = signal<WebSocket | null>(null);
const connectionStatus = signal<'connecting' | 'connected' | 'disconnected'>('disconnected');

export default function ChatRoom({ 
  roomId, 
  playerId, 
  playerName, 
  currentWord, 
  isCurrentDrawer = false 
}: ChatRoomProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputMessage, setInputMessage] = useState('');
  const [isTyping, setIsTyping] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // WebSocket connection management
  useEffect(() => {
    // Skip WebSocket in development environment
    if (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1') {
      console.log('WebSocket disabled in development environment');
      connectionStatus.value = 'disconnected';
      return;
    }

    let ws: WebSocket | null = null;
    let reconnectTimeout: number | null = null;
    let reconnectAttempts = 0;
    const maxReconnectAttempts = 5;

    const connectWebSocket = () => {
      try {
        connectionStatus.value = 'connecting';
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/api/websocket?roomId=${roomId}`;
        
        ws = new WebSocket(wsUrl);
        wsConnection.value = ws;

        ws.onopen = () => {
          console.log('Chat WebSocket connected');
          connectionStatus.value = 'connected';
          reconnectAttempts = 0;
          
          // Join room
          ws?.send(JSON.stringify({
            type: 'join-room',
            roomId,
            playerId,
            data: { playerName }
          }));
        };

        ws.onmessage = (event) => {
          try {
            const message = JSON.parse(event.data);
            
            if (message.type === 'chat-message') {
              const chatMessage: ChatMessage = message.data;
              setMessages(prev => [...prev, chatMessage]);
            } else if (message.type === 'room-update' && message.data?.type === 'error') {
              console.error('Chat error:', message.data.message);
            }
          } catch (error) {
            console.error('Error parsing chat message:', error);
          }
        };

        ws.onclose = () => {
          console.log('Chat WebSocket disconnected');
          connectionStatus.value = 'disconnected';
          wsConnection.value = null;
          
          // Don't attempt to reconnect in development
          if (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1') {
            return;
          }
          
          // Attempt to reconnect with exponential backoff
          if (reconnectAttempts < maxReconnectAttempts) {
            const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 30000);
            reconnectTimeout = setTimeout(() => {
              reconnectAttempts++;
              connectWebSocket();
            }, delay);
          }
        };

        ws.onerror = (error) => {
          console.error('Chat WebSocket error:', error);
          connectionStatus.value = 'disconnected';
        };
      } catch (error) {
        console.error('Failed to connect chat WebSocket:', error);
        connectionStatus.value = 'disconnected';
      }
    };

    connectWebSocket();

    return () => {
      if (reconnectTimeout) {
        clearTimeout(reconnectTimeout);
      }
      if (ws) {
        ws.close();
      }
    };
  }, [roomId, playerId, playerName]);

  const sendMessage = (e: Event) => {
    e.preventDefault();
    
    const message = inputMessage.trim();
    if (!message || message.length > 200) return;
    
    const ws = wsConnection.value;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      console.error('WebSocket not connected');
      return;
    }

    try {
      ws.send(JSON.stringify({
        type: 'chat',
        roomId,
        playerId,
        data: { text: message }
      }));
      
      setInputMessage('');
      inputRef.current?.focus();
    } catch (error) {
      console.error('Error sending message:', error);
    }
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
      {/* Connection status */}
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-lg font-semibold text-gray-800">Chat</h2>
        <div class="flex items-center space-x-2">
          <div class={`w-2 h-2 rounded-full ${
            connectionStatus.value === 'connected' ? 'bg-green-500' : 
            connectionStatus.value === 'connecting' ? 'bg-yellow-500' : 'bg-red-500'
          }`}></div>
          <span class="text-xs text-gray-500">
            {connectionStatus.value === 'connected' ? 'Connected' : 
             connectionStatus.value === 'connecting' ? 'Connecting...' : 'Offline'}
          </span>
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
                  class={`max-w-xs px-3 py-2 rounded-lg text-sm ${
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

      {/* Message input */}
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
          disabled={connectionStatus.value !== 'connected'}
          class="flex-1 px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-gray-100 disabled:text-gray-500 text-sm"
        />
        <button
          type="submit"
          disabled={
            !inputMessage.trim() || 
            inputMessage.length > 200 || 
            connectionStatus.value !== 'connected'
          }
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium"
        >
          Send
        </button>
      </form>

      {/* Character count */}
      <div class="text-xs text-gray-500 mt-1 text-right">
        {inputMessage.length}/200
      </div>
    </div>
  );
}