// Core chat room component - game agnostic
import { useEffect, useRef, useState } from "preact/hooks";
import { ChatMessage } from "../../types/core/room.ts";

interface ChatRoomProps {
  roomId: string;
  messages?: ChatMessage[];
  onSendMessage?: (message: string) => void;
  currentPlayerId: string;
  currentPlayerName?: string;
  disabled?: boolean;
  placeholder?: string;
}

export default function ChatRoom({
  roomId,
  messages,
  onSendMessage,
  currentPlayerId,
  currentPlayerName,
  disabled = false,
  placeholder = "Type a message...",
}: ChatRoomProps) {
  const [inputValue, setInputValue] = useState("");
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>(messages ?? []);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const didInitFromProp = useRef(false);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  // Initialize from messages prop once (if provided)
  useEffect(() => {
    if (!didInitFromProp.current && messages && messages.length) {
      setChatMessages(messages);
      didInitFromProp.current = true;
    }
  }, [messages]);

  // Auto-scroll on new messages
  useEffect(() => {
    scrollToBottom();
  }, [chatMessages]);

  // Listen for forwarded WebSocket chat messages from Scoreboard
  useEffect(() => {
    const handler = (e: Event) => {
      const ce = e as CustomEvent;
      const payload = ce.detail?.data;
      if (payload?.type === "chat-message" && payload?.data) {
        console.log("ChatRoom: received chat-message via event", payload.data);
        const incoming = payload.data as ChatMessage;
        setChatMessages((prev) => [...prev, incoming]);
      }
    };
    globalThis.addEventListener("websocket-message", handler as EventListener);
    return () => globalThis.removeEventListener("websocket-message", handler as EventListener);
  }, []);

  const handleSubmit = (e: Event) => {
    e.preventDefault();
    if (inputValue.trim() && !disabled) {
      const text = inputValue.trim();

      if (typeof onSendMessage === "function") {
        try {
          onSendMessage(text);
        } catch (err) {
          console.error("ChatRoom: onSendMessage threw:", err);
        }
      } else {
        // Fallback: send via global WebSocket set by Scoreboard
        const ws = (globalThis as any).__gameWebSocket as WebSocket | undefined;
        if (ws && ws.readyState === WebSocket.OPEN) {
          if (!currentPlayerId) {
            console.warn("ChatRoom: currentPlayerId is empty; message will use empty playerId");
          }
          console.log("ChatRoom: sending chat via WebSocket", {
            roomId,
            playerId: currentPlayerId,
            playerName: currentPlayerName,
            text,
            readyState: ws.readyState,
          });
          ws.send(
            JSON.stringify({
              type: "chat",
              roomId,
              playerId: currentPlayerId,
              data: { text, playerName: currentPlayerName },
            }),
          );
        } else {
          console.warn("ChatRoom: WebSocket not available for sending chat message");
        }
      }

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
        {chatMessages.length === 0
          ? (
            <div class="text-center text-gray-500 text-sm py-8">
              No messages yet. Start the conversation!
            </div>
          )
          : (
            chatMessages.map((message) => (
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
