import { useState } from "preact/hooks";
import type { JSX } from "preact";

interface GameSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: (settings: GameSettings) => void;
  currentSettings: GameSettings;
}

export interface GameSettings {
  maxRounds: number;
  roundTimeMinutes: number;
  roundTimeSeconds: number;
}

export default function GameSettingsModal({
  isOpen,
  onClose,
  onSave,
  currentSettings,
}: GameSettingsModalProps) {
  const [settings, setSettings] = useState<GameSettings>(currentSettings);

  const handleSave = () => {
    onSave(settings);
    onClose();
  };

  const handleCancel = () => {
    setSettings(currentSettings); // Reset to original settings
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg shadow-xl p-6 w-full max-w-md mx-4">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-semibold text-gray-800">Game Settings</h2>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600 transition-colors"
          >
            <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>

        <div className="space-y-4">
          {/* Max Rounds */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Maximum Rounds
            </label>
            <select
              value={settings.maxRounds}
              onChange={(e) =>
                setSettings((prev) => ({
                  ...prev,
                  maxRounds: parseInt((e.target as HTMLSelectElement).value),
                }))}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            >
              <option value={3}>3 Rounds</option>
              <option value={5}>5 Rounds</option>
              <option value={7}>7 Rounds</option>
              <option value={10}>10 Rounds</option>
            </select>
          </div>

          {/* Round Time */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Round Time
            </label>
            <div className="flex space-x-2">
              <div className="flex-1">
                <select
                  value={settings.roundTimeMinutes}
                  onChange={(e) =>
                    setSettings((prev) => ({
                      ...prev,
                      roundTimeMinutes: parseInt((e.target as HTMLSelectElement).value),
                    }))}
                  className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                >
                  <option value={0}>0</option>
                  <option value={1}>1</option>
                  <option value={2}>2</option>
                  <option value={3}>3</option>
                  <option value={4}>4</option>
                  <option value={5}>5</option>
                </select>
                <label className="block text-xs text-gray-500 mt-1">Minutes</label>
              </div>
              <div className="flex-1">
                <select
                  value={settings.roundTimeSeconds}
                  onChange={(e) =>
                    setSettings((prev) => ({
                      ...prev,
                      roundTimeSeconds: parseInt((e.target as HTMLSelectElement).value),
                    }))}
                  className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                >
                  <option value={0}>00</option>
                  <option value={15}>15</option>
                  <option value={30}>30</option>
                  <option value={45}>45</option>
                </select>
                <label className="block text-xs text-gray-500 mt-1">Seconds</label>
              </div>
            </div>
            <div className="text-xs text-gray-500 mt-1">
              Total:{" "}
              {settings.roundTimeMinutes}:{settings.roundTimeSeconds.toString().padStart(2, "0")}
            </div>
          </div>

          {/* Settings Preview */}
          <div className="bg-gray-50 rounded-lg p-3">
            <h3 className="text-sm font-medium text-gray-700 mb-2">Game Preview</h3>
            <div className="text-sm text-gray-600 space-y-1">
              <div>• {settings.maxRounds} rounds total</div>
              <div>
                •{" "}
                {settings.roundTimeMinutes}:{settings.roundTimeSeconds.toString().padStart(2, "0")}
                {" "}
                per round
              </div>
              <div>
                • Estimated game time: ~{Math.ceil(
                  settings.maxRounds *
                    (settings.roundTimeMinutes + settings.roundTimeSeconds / 60) * 1.5,
                )} minutes
              </div>
            </div>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex space-x-3 mt-6">
          <button
            onClick={handleCancel}
            className="flex-1 px-4 py-2 border border-gray-300 text-gray-700 rounded-lg hover:bg-gray-50 transition-colors duration-200"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            className="flex-1 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors duration-200"
          >
            Save Settings
          </button>
        </div>
      </div>
    </div>
  );
}
