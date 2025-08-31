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
    <div class="flex flex-col h-full w-full relative">
      {/* Chat Header */}
      <div class="px-4 py-3 border-b border-white/20 flex-shrink-0">
        <h3 class="text-sm font-medium text-white">Chat</h3>
      </div>

      {/* Messages - with bottom padding to account for fixed input */}
      <div class="flex-1 overflow-y-auto p-4 space-y-2 min-h-0 w-full pb-20">
        {chatMessages.length === 0
          ? (
            <div class="text-center text-white/60 text-sm py-8">
              No messages yet. Start the conversation!
            </div>
          )
          : (
            chatMessages.map((message) => (
              <div
                key={message.id}
                class={`flex w-full ${
                  message.playerId === currentPlayerId ? "justify-end" : "justify-start"
                }`}
              >
                <div
                  class={`max-w-xs lg:max-w-md px-3 py-2 rounded-lg text-sm ${
                    message.playerId === currentPlayerId
                      ? "bg-blue-500 text-white"
                      : message.isSystemMessage
                      ? "bg-yellow-400/20 text-yellow-100 border border-yellow-400/30"
                      : "bg-white/20 text-white backdrop-blur-sm"
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

      {/* Fixed Input at Bottom */}
      <div class="absolute bottom-0 left-0 right-0 p-4 border-t border-white/20 bg-white/5 backdrop-blur-md">
        <form onSubmit={handleSubmit} class="flex space-x-2 w-full">
          <input
            ref={inputRef}
            type="text"
            value={inputValue}
            onInput={(e) => setInputValue((e.target as HTMLInputElement).value)}
            onKeyPress={handleKeyPress}
            placeholder={placeholder}
            disabled={disabled}
            class="flex-1 px-3 py-2 bg-white/20 border border-white/30 rounded-md text-sm text-white placeholder-white/60 focus:outline-none focus:ring-2 focus:ring-white/50 focus:border-white/50 disabled:bg-white/10 disabled:cursor-not-allowed backdrop-blur-sm"
            maxLength={100}
          />
          <button
            type="submit"
            disabled={!inputValue.trim() || disabled}
            class="px-4 py-2 bg-white text-purple-600 text-sm font-medium rounded-md hover:bg-white/90 focus:outline-none focus:ring-2 focus:ring-white/50 focus:ring-offset-2 disabled:bg-white/20 disabled:text-white/50 disabled:cursor-not-allowed transition-colors"
          >
            Send
          </button>
        </form>
      </div>
    </div>
  );
}
