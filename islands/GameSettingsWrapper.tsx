import { useState } from "preact/hooks";
import GameSettingsModal, { type GameSettings } from "../components/GameSettingsModal.tsx";

interface GameSettingsWrapperProps {
  roomId: string;
  playerId: string;
  onShowModal: boolean;
  onModalClose: () => void;
}

export default function GameSettingsWrapper({
  roomId,
  playerId,
  onShowModal,
  onModalClose,
}: GameSettingsWrapperProps) {
  const [gameSettings, setGameSettings] = useState<GameSettings>({
    maxRounds: 5,
    roundTimeSeconds: 75,
  });

  // Handle settings save
  const handleSettingsSave = (settings: GameSettings) => {
    setGameSettings(settings);
    // Send settings update via WebSocket
    const ws = (globalThis as any).__gameWebSocket as WebSocket;
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: "update-settings",
        roomId,
        playerId,
        data: settings,
      }));
    }
  };

  return (
    <GameSettingsModal
      isOpen={onShowModal}
      onClose={onModalClose}
      onSave={handleSettingsSave}
      currentSettings={gameSettings}
    />
  );
}