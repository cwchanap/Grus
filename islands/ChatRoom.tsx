import { useEffect, useRef, useState } from "preact/hooks";
import { useComputed } from "@preact/signals";
import type { JSX } from "preact";
import type { ChatMessage } from "../types/game.ts";

// Extend ChatMessage type for offline support
interface ExtendedChatMessage extends ChatMessage {
  isOffline?: boolean;
}
import {
  connectionState,
} from "../lib/websocket/connection-manager.ts";
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
  isCurrentDrawer = false,
}: ChatRoomProps) {
  const [messages, setMessages] = useState<ExtendedChatMessage[]>([]);
  const [isTyping, setIsTyping] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [buttonEnabled, setButtonEnabled] = useState(false);
  const [charCount, setCharCount] = useState(0);

  const offlineManagerRef = useRef<OfflineManager | null>(null);

  const _connectionStatus = useComputed(() => connectionState.value);
  const offlineStatus = useComputed(() => offlineState.value);

  // Debug logging
  console.log("ChatRoom: Rendering with buttonEnabled:", buttonEnabled, "charCount:", charCount, "messages:", messages.length);

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // Use shared WebSocket connection instead of creating our own
  useEffect(() => {
    // Initialize offline manager
    offlineManagerRef.current = new OfflineManager(roomId);

    // Listen for chat messages from the shared WebSocket connection
    const handleChatMessage = (event: Event) => {
      const customEvent = event as CustomEvent;
      const { data } = customEvent.detail;
      if (data && data.type === "chat-message") {
        const chatMessage: ChatMessage = data.data;
        console.log("ChatRoom: Received WebSocket message:", chatMessage);
        setMessages((prev) => {
          const newMessages = [...prev, chatMessage];
          console.log("ChatRoom: Updated messages from WebSocket, length:", newMessages.length);
          
          // Only add to DOM if it's not our own message (we already added it optimistically)
          if (chatMessage.playerId !== playerId) {
            addMessageToDOM(chatMessage);
          } else {
            // For our own messages, update the existing DOM message to remove offline indicator
            updateMessageInDOM(chatMessage);
          }
          
          return newMessages;
        });
      }
    };

    // Listen for global WebSocket messages
    globalThis.addEventListener("websocket-message", handleChatMessage);

    return () => {
      offlineManagerRef.current?.destroy();
      globalThis.removeEventListener("websocket-message", handleChatMessage);
    };
  }, [roomId]);

  // Function to update existing message in DOM (remove offline indicator)
  const updateMessageInDOM = (message: ChatMessage) => {
    const chatContainer = document.querySelector('.bg-gray-50.rounded-lg.p-3.mb-3.overflow-y-auto.min-h-0');
    const messagesContainer = chatContainer?.querySelector('.space-y-2');
    if (!messagesContainer) return;

    // Find the message with offline indicator and update it
    const messageElements = messagesContainer.querySelectorAll('.flex.flex-col.space-y-1');
    for (const messageEl of messageElements) {
      const offlineIndicator = messageEl.querySelector('span.ml-2.text-xs.opacity-75');
      if (offlineIndicator && offlineIndicator.textContent === '⏳') {
        // Update the message styling to remove offline indicator and change color
        const messageContent = messageEl.querySelector('.max-w-xs, .max-w-sm');
        if (messageContent) {
          messageContent.className = messageContent.className.replace(
            'bg-gray-400 text-white opacity-75',
            'bg-blue-500 text-white'
          );
          offlineIndicator.remove();
          console.log("ChatRoom: Updated message in DOM to remove offline indicator");
          break;
        }
      }
    }
  };

  // Function to add message directly to DOM
  const addMessageToDOM = (message: ExtendedChatMessage) => {
    // Find the chat messages container more specifically
    const chatContainer = document.querySelector('.bg-gray-50.rounded-lg.p-3.mb-3.overflow-y-auto.min-h-0');
    const messagesContainer = chatContainer?.querySelector('.space-y-2');
    if (!messagesContainer) {
      console.log("ChatRoom: Could not find chat messages container");
      return;
    }

    // Remove empty state if it exists
    const emptyState = messagesContainer.querySelector('.text-center.text-gray-500.py-8');
    if (emptyState) {
      emptyState.remove();
    }

    // Create message element
    const messageDiv = document.createElement('div');
    messageDiv.className = `flex flex-col space-y-1 ${
      message.playerId === playerId ? 'items-end' : 'items-start'
    }`;

    const isOwn = message.playerId === playerId;
    const messageContent = `
      <div class="max-w-xs sm:max-w-sm px-3 py-2 rounded-lg text-sm ${
        isOwn
          ? message.isOffline 
            ? 'bg-gray-400 text-white opacity-75'
            : 'bg-blue-500 text-white'
          : 'bg-white border border-gray-200 text-gray-800'
      }">
        ${!isOwn ? `<div class="text-xs font-medium mb-1 opacity-75">${message.playerName}</div>` : ''}
        <div class="break-words">
          ${message.message}
          ${message.isOffline && isOwn ? '<span class="ml-2 text-xs opacity-75">⏳</span>' : ''}
        </div>
      </div>
      <div class="text-xs text-gray-400">
        ${formatTime(message.timestamp)}
      </div>
    `;

    messageDiv.innerHTML = messageContent;
    messagesContainer.appendChild(messageDiv);

    // Scroll to bottom
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    
    console.log("ChatRoom: Message added to DOM:", message.message);
  };

  const sendMessage = (e: JSX.TargetedEvent<HTMLFormElement, Event>) => {
    e.preventDefault();

    // Get message directly from DOM
    const message = (inputRef.current?.value || "").trim();
    if (!message || message.length > 200) return;

    // Generate a browser-safe unique ID
    const generateId = () => {
      return Date.now().toString(36) + Math.random().toString(36).substr(2, 9);
    };

    const chatMessage: ChatMessage = {
      id: generateId(),
      playerId,
      playerName,
      message,
      timestamp: Date.now(),
      isGuess: false,
      isCorrect: false,
    };

    // Always show the message locally first (optimistic update)
    console.log("ChatRoom: Adding message locally:", chatMessage);
    setMessages((prev) => {
      const newMessages = [...prev, { ...chatMessage, isOffline: true }];
      console.log("ChatRoom: New messages array length:", newMessages.length);
      
      // DIRECT DOM MANIPULATION to ensure message appears immediately
      addMessageToDOM({ ...chatMessage, isOffline: true });
      
      return newMessages;
    });

    // Try to send message via shared WebSocket
    const ws = (globalThis as any).__gameWebSocket as WebSocket;
    if (ws && ws.readyState === WebSocket.OPEN) {
      console.log("ChatRoom: Sending chat message via shared WebSocket");
      ws.send(JSON.stringify({
        type: "chat",
        roomId,
        playerId,
        data: { text: message },
      }));
      
      // Update the message to remove offline indicator after successful send
      setTimeout(() => {
        setMessages((prev) => prev.map(msg => 
          msg.id === chatMessage.id ? { ...msg, isOffline: false } : msg
        ));
      }, 100);
    } else {
      console.log("ChatRoom: WebSocket not available, message will remain as offline");
      
      // Try to queue for later if offline manager is available
      const offlineManager = offlineManagerRef.current;
      if (offlineManager) {
        offlineManager.queueChatMessage(chatMessage);
      }
    }

    if (inputRef.current) {
      inputRef.current.value = "";
    }
    
    // Update React state
    setButtonEnabled(false);
    setCharCount(0);
    
    // DIRECT DOM MANIPULATION to ensure UI resets immediately
    const button = inputRef.current?.closest('form')?.querySelector('button[type="submit"]') as HTMLButtonElement;
    const charCountElement = inputRef.current?.closest('form')?.parentElement?.querySelector('.text-xs.text-gray-500') as HTMLElement;
    
    if (button) {
      button.disabled = true;
      console.log("ChatRoom: Button directly disabled after send");
    }
    
    if (charCountElement) {
      charCountElement.textContent = "0/200";
      console.log("ChatRoom: Character count reset to 0");
    }
    
    inputRef.current?.focus();
  };

  const handleInputChange = (e: JSX.TargetedEvent<HTMLInputElement, Event>) => {
    const value = e.currentTarget.value;
    console.log("ChatRoom: Input change event fired", value);
    
    // Update UI state based on input value
    const trimmedValue = value.trim();
    const isValid = trimmedValue.length > 0 && value.length <= 200;
    
    // Update React state (even though it might not trigger re-render)
    setButtonEnabled(isValid);
    setCharCount(value.length);
    
    // DIRECT DOM MANIPULATION to ensure UI updates immediately
    const form = e.currentTarget.closest('form');
    const button = form?.querySelector('button[type="submit"]') as HTMLButtonElement;
    const charCountElement = form?.parentElement?.querySelector('.text-xs.text-gray-500') as HTMLElement;
    
    if (button) {
      button.disabled = !isValid;
      console.log("ChatRoom: Button directly", isValid ? "enabled" : "disabled");
    }
    
    if (charCountElement) {
      charCountElement.textContent = `${value.length}/200`;
      console.log("ChatRoom: Character count updated to:", value.length);
    }

    // Show typing indicator briefly
    setIsTyping(true);
    setTimeout(() => setIsTyping(false), 1000);
  };

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
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
        <h2 class="text-base sm:text-lg font-semibold text-gray-800">
          {currentWord && !isCurrentDrawer ? "Chat & Guessing" : "Chat"}
        </h2>
        <div class="flex items-center space-x-2 sm:space-x-3">
          <ConnectionStatus size="sm" />
          {offlineStatus.value.isOffline && offlineStatus.value.pendingMessages.length > 0 && (
            <div class="text-xs text-orange-600 bg-orange-50 px-2 py-1 rounded">
              <span class="hidden sm:inline">
                {offlineStatus.value.pendingMessages.length} pending
              </span>
              <span class="sm:hidden">{offlineStatus.value.pendingMessages.length}</span>
            </div>
          )}
        </div>
      </div>

      {/* Messages area */}
      <div class="flex-1 bg-gray-50 rounded-lg p-3 mb-3 overflow-y-auto min-h-0">
        <div class="space-y-2">
          {messages.length === 0
            ? (
              <div class="text-center text-gray-500 py-8">
                <div class="text-2xl mb-2">💬</div>
                <p class="text-sm">
                  {currentWord && isCurrentDrawer
                    ? "Others will guess your drawing here!"
                    : currentWord && !isCurrentDrawer
                    ? "Start guessing what's being drawn!"
                    : "Chat with other players!"}
                </p>
              </div>
            )
            : (
              messages.map((message) => (
                <div
                  key={message.id}
                  class={`flex flex-col space-y-1 ${
                    isOwnMessage(message) ? "items-end" : "items-start"
                  }`}
                >
                  <div
                    class={`max-w-xs sm:max-w-sm px-3 py-2 rounded-lg text-sm ${
                      isCorrectGuess(message)
                        ? "bg-green-100 border border-green-300 text-green-800"
                        : isOwnMessage(message)
                        ? message.isOffline 
                          ? "bg-gray-400 text-white opacity-75"
                          : "bg-blue-500 text-white"
                        : "bg-white border border-gray-200 text-gray-800"
                    }`}
                  >
                    {!isOwnMessage(message) && (
                      <div class="text-xs font-medium mb-1 opacity-75">
                        {message.playerName}
                        {isCorrectGuess(message) && " 🎉"}
                      </div>
                    )}
                    <div class="break-words">
                      {message.message}
                      {message.isOffline && isOwnMessage(message) && (
                        <span class="ml-2 text-xs opacity-75">⏳</span>
                      )}
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
                <div
                  class="w-1 h-1 bg-gray-400 rounded-full animate-bounce"
                  style="animation-delay: 0.1s"
                >
                </div>
                <div
                  class="w-1 h-1 bg-gray-400 rounded-full animate-bounce"
                  style="animation-delay: 0.2s"
                >
                </div>
              </div>
              <span>Someone is typing...</span>
            </div>
          )}
          <div ref={messagesEndRef} />
        </div>
      </div>

      {/* Guess hint for non-drawers during drawing phase */}
      {shouldShowGuessHint() && (
        <div class="bg-blue-50 border border-blue-200 rounded-lg p-2 mb-3">
          <div class="text-xs text-blue-700 font-medium">
            💡 Tip: Type your guess and press Enter!
          </div>
        </div>
      )}

      {/* General chat hint when not in drawing phase */}
      {!currentWord && messages.length === 0 && (
        <div class="bg-green-50 border border-green-200 rounded-lg p-2 mb-3">
          <div class="text-xs text-green-700 font-medium">
            💬 Chat is always available! Say hello to other players.
          </div>
        </div>
      )}

      {/* Message input - Mobile optimized */}
      <form onSubmit={sendMessage} class="flex space-x-2">
        <input
          ref={inputRef}
          type="text"
          onInput={handleInputChange}
          placeholder={currentWord && isCurrentDrawer
            ? "You're drawing! Others will guess..."
            : currentWord && !isCurrentDrawer
            ? "Type your guess here..."
            : "Type a message..."}
          maxLength={200}
          disabled={false} // Allow typing in offline mode
          class="flex-1 px-3 py-2 sm:py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-sm touch-manipulation"
        />
        <button
          type="submit"
          disabled={!buttonEnabled}
          class="px-3 sm:px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium touch-manipulation"
        >
          <span class="hidden sm:inline">Send</span>
          <span class="sm:hidden">→</span>
        </button>
      </form>

      {/* Character count */}
      <div class="text-xs text-gray-500 mt-1 text-right">
        {charCount}/200
      </div>
    </div>
  );
}

// Export directly without error boundary for testing
export default function ChatRoom(props: ChatRoomProps) {
  return <ChatRoomComponent key={`${props.roomId}-${props.playerId}`} {...props} />;
}
