import { useEffect, useState } from "preact/hooks";
import GameSettingsModal, { type GameSettings } from "../components/GameSettingsModal.tsx";

interface GameSettingsWrapperProps {
  roomId: string;
  playerId: string;
  isOpen: boolean;
  onModalClose: () => void;
  currentSettings?: GameSettings;
}

export default function GameSettingsWrapper({
  roomId,
  playerId,
  isOpen,
  onModalClose,
  currentSettings,
}: GameSettingsWrapperProps) {
  console.log("GameSettingsWrapper render", { isOpen, currentSettings, roomId, playerId });

  const [gameSettings, setGameSettings] = useState<GameSettings>(
    currentSettings || {
      maxRounds: 5,
      roundTimeSeconds: 75,
    },
  );

  // Sync with currentSettings prop changes
  useEffect(() => {
    if (currentSettings) {
      console.log("GameSettingsWrapper: Updating settings from prop:", currentSettings);
      setGameSettings(currentSettings);
    }
  }, [currentSettings]);

  // Listen for settings updates from server
  useEffect(() => {
    const handleSettingsUpdate = (event: CustomEvent) => {
      const { settings } = event.detail;
      console.log("GameSettingsWrapper: Received settings update:", settings);
      setGameSettings((prevSettings) => ({
        ...prevSettings,
        ...settings,
      }));
    };

    globalThis.addEventListener("settings-updated", handleSettingsUpdate as EventListener);

    return () => {
      globalThis.removeEventListener("settings-updated", handleSettingsUpdate as EventListener);
    };
  }, []);

  // Handle settings save
  const handleSettingsSave = (settings: GameSettings) => {
    console.log("GameSettingsWrapper: Saving settings:", settings);
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
    } else {
      console.warn("GameSettingsWrapper: WebSocket not available for settings update");
    }
  };

  return (
    <GameSettingsModal
      isOpen={isOpen}
      onClose={onModalClose}
      onSave={handleSettingsSave}
      currentSettings={gameSettings}
    />
  );
}
