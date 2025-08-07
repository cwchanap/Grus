// Core chat room component - game agnostic
import { useEffect, useRef, useState } from "preact/hooks";
import { ChatMessage } from "../../types/core/room.ts";

interface ChatRoomProps {
  messages: ChatMessage[];
  onSendMessage: (message: string) => void;
  currentPlayerId: string;
  disabled?: boolean;
  placeholder?: string;
}

export default function ChatRoom({
  messages,
  onSendMessage,
  currentPlayerId,
  disabled = false,
  placeholder = "Type a message...",
}: ChatRoomProps) {
  const [inputValue, setInputValue] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  useEffect(() => {
    scrollToBottom();
  }, [messages]);

  const handleSubmit = (e: Event) => {
    e.preventDefault();
    if (inputValue.trim() && !disabled) {
      onSendMessage(inputValue.trim());
      setInputValue("");
    }
  };

  const handleKeyPress = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  };

  return (
    <div class="flex flex-col h-full bg-white rounded-lg shadow-sm border border-gray-200">
      {/* Chat Header */}
      <div class="px-4 py-3 border-b border-gray-200 bg-gray-50 rounded-t-lg">
        <h3 class="text-sm font-medium text-gray-900">Chat</h3>
      </div>

      {/* Messages */}
      <div class="flex-1 overflow-y-auto p-4 space-y-2 min-h-0">
        {messages.length === 0
          ? (
            <div class="text-center text-gray-500 text-sm py-8">
              No messages yet. Start the conversation!
            </div>
          )
          : (
            messages.map((message) => (
              <div
                key={message.id}
                class={`flex ${
                  message.playerId === currentPlayerId ? "justify-end" : "justify-start"
                }`}
              >
                <div
                  class={`max-w-xs lg:max-w-md px-3 py-2 rounded-lg text-sm ${
                    message.playerId === currentPlayerId
                      ? "bg-blue-500 text-white"
                      : message.isSystemMessage
                      ? "bg-yellow-100 text-yellow-800 border border-yellow-200"
                      : "bg-gray-100 text-gray-900"
                  }`}
                >
                  {message.playerId !== currentPlayerId && !message.isSystemMessage && (
                    <div class="text-xs opacity-75 mb-1">{message.playerName}</div>
                  )}
                  <div class="break-words">{message.message}</div>
                  <div class="text-xs opacity-75 mt-1">
                    {new Date(message.timestamp).toLocaleTimeString([], {
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </div>
                </div>
              </div>
            ))
          )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div class="p-4 border-t border-gray-200">
        <form onSubmit={handleSubmit} class="flex space-x-2">
          <input
            ref={inputRef}
            type="text"
            value={inputValue}
            onInput={(e) => setInputValue((e.target as HTMLInputElement).value)}
            onKeyPress={handleKeyPress}
            placeholder={placeholder}
            disabled={disabled}
            class="flex-1 px-3 py-2 border border-gray-300 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-gray-100 disabled:cursor-not-allowed"
            maxLength={100}
          />
          <button
            type="submit"
            disabled={!inputValue.trim() || disabled}
            class="px-4 py-2 bg-blue-500 text-white text-sm font-medium rounded-md hover:bg-blue-600 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:bg-gray-300 disabled:cursor-not-allowed transition-colors"
          >
            Send
          </button>
        </form>
      </div>
    </div>
  );
}
