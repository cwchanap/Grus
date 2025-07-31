// Simple WebSocket test page
import { Head } from "$fresh/runtime.ts";

export default function WebSocketTest() {
  return (
    <>
      <Head>
        <title>WebSocket Test</title>
      </Head>
      <div class="container mx-auto p-4">
        <h1 class="text-2xl font-bold mb-4">WebSocket Connection Test</h1>
        
        <div class="space-y-4">
          <div>
            <button 
              id="connect-btn" 
              class="bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600"
            >
              Connect to WebSocket
            </button>
            <button 
              id="disconnect-btn" 
              class="bg-red-500 text-white px-4 py-2 rounded hover:bg-red-600 ml-2"
              disabled
            >
              Disconnect
            </button>
          </div>
          
          <div>
            <input 
              id="message-input" 
              type="text" 
              placeholder="Enter test message" 
              class="border border-gray-300 px-3 py-2 rounded w-64"
            />
            <button 
              id="send-btn" 
              class="bg-green-500 text-white px-4 py-2 rounded hover:bg-green-600 ml-2"
              disabled
            >
              Send Message
            </button>
          </div>
          
          <div>
            <h3 class="font-semibold">Connection Status:</h3>
            <div id="status" class="text-gray-600">Disconnected</div>
          </div>
          
          <div>
            <h3 class="font-semibold">Messages:</h3>
            <div id="messages" class="border border-gray-300 p-3 h-64 overflow-y-auto bg-gray-50"></div>
          </div>
        </div>
      </div>

      <script dangerouslySetInnerHTML={{
        __html: `
          let ws = null;
          const connectBtn = document.getElementById('connect-btn');
          const disconnectBtn = document.getElementById('disconnect-btn');
          const sendBtn = document.getElementById('send-btn');
          const messageInput = document.getElementById('message-input');
          const status = document.getElementById('status');
          const messages = document.getElementById('messages');

          function addMessage(message, type = 'info') {
            const div = document.createElement('div');
            div.className = type === 'error' ? 'text-red-600' : type === 'success' ? 'text-green-600' : 'text-blue-600';
            div.textContent = new Date().toLocaleTimeString() + ': ' + message;
            messages.appendChild(div);
            messages.scrollTop = messages.scrollHeight;
          }

          function updateStatus(statusText, isConnected) {
            status.textContent = statusText;
            status.className = isConnected ? 'text-green-600' : 'text-red-600';
            connectBtn.disabled = isConnected;
            disconnectBtn.disabled = !isConnected;
            sendBtn.disabled = !isConnected;
          }

          connectBtn.addEventListener('click', () => {
            try {
              const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
              const wsUrl = protocol + '//' + window.location.host + '/api/websocket-test';
              
              addMessage('Connecting to: ' + wsUrl);
              ws = new WebSocket(wsUrl);

              ws.onopen = () => {
                addMessage('WebSocket connected successfully!', 'success');
                updateStatus('Connected', true);
              };

              ws.onmessage = (event) => {
                try {
                  const data = JSON.parse(event.data);
                  addMessage('Received: ' + JSON.stringify(data, null, 2), 'success');
                } catch (e) {
                  addMessage('Received (raw): ' + event.data, 'success');
                }
              };

              ws.onclose = (event) => {
                addMessage('WebSocket closed. Code: ' + event.code + ', Reason: ' + event.reason, 'error');
                updateStatus('Disconnected', false);
                ws = null;
              };

              ws.onerror = (error) => {
                addMessage('WebSocket error: ' + error, 'error');
                updateStatus('Error', false);
              };
            } catch (error) {
              addMessage('Failed to create WebSocket: ' + error, 'error');
            }
          });

          disconnectBtn.addEventListener('click', () => {
            if (ws) {
              ws.close();
            }
          });

          sendBtn.addEventListener('click', () => {
            const message = messageInput.value.trim();
            if (message && ws && ws.readyState === WebSocket.OPEN) {
              try {
                const testMessage = {
                  type: 'test',
                  message: message,
                  timestamp: Date.now()
                };
                ws.send(JSON.stringify(testMessage));
                addMessage('Sent: ' + JSON.stringify(testMessage));
                messageInput.value = '';
              } catch (error) {
                addMessage('Failed to send message: ' + error, 'error');
              }
            }
          });

          messageInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
              sendBtn.click();
            }
          });
        `
      }} />
    </>
  );
}