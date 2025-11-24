import { useState } from "preact/hooks";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog.tsx";
import { Button } from "./ui/button.tsx";
import { Select, SelectItem } from "./ui/select.tsx";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card.tsx";

interface GameSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: (settings: GameSettings) => void;
  currentSettings: GameSettings;
}

export interface GameSettings {
  maxRounds: number;
  roundTimeSeconds: number; // Total seconds (60-90 range as per product rules)
}

export default function GameSettingsModal({
  isOpen,
  onClose,
  onSave,
  currentSettings,
}: GameSettingsModalProps) {
  console.log("GameSettingsModal render", { isOpen, currentSettings });
  const [settings, setSettings] = useState<GameSettings>(currentSettings);

  const handleSave = () => {
    onSave(settings);
    onClose();
  };

  const handleCancel = () => {
    setSettings(currentSettings); // Reset to original settings
    onClose();
  };

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        onClose={onClose}
        className="sm:max-w-md"
        role="dialog"
        aria-modal="true"
        data-testid="game-settings-modal"
      >
        <DialogHeader>
          <DialogTitle>Game Settings</DialogTitle>
          <DialogDescription>
            Configure the game rules and timing for your room.
          </DialogDescription>
        </DialogHeader>

        <div className="px-6 py-4 space-y-4">
          {/* Max Rounds */}
          <div className="space-y-2">
            <label className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70">
              Maximum Rounds
            </label>
            <Select
              value={settings.maxRounds.toString()}
              onChange={(e) =>
                setSettings((prev) => ({
                  ...prev,
                  maxRounds: parseInt((e.target as HTMLSelectElement).value),
                }))}
            >
              <SelectItem value="3">3 Rounds</SelectItem>
              <SelectItem value="5">5 Rounds</SelectItem>
              <SelectItem value="7">7 Rounds</SelectItem>
              <SelectItem value="10">10 Rounds</SelectItem>
            </Select>
          </div>

          {/* Round Time */}
          <div className="space-y-2">
            <label className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70">
              Round Time (60-90 seconds)
            </label>
            <Select
              value={settings.roundTimeSeconds.toString()}
              onChange={(e) =>
                setSettings((prev) => ({
                  ...prev,
                  roundTimeSeconds: parseInt((e.target as HTMLSelectElement).value),
                }))}
            >
              <SelectItem value="60">1:00 (60 seconds)</SelectItem>
              <SelectItem value="75">1:15 (75 seconds) - Default</SelectItem>
              <SelectItem value="90">1:30 (90 seconds)</SelectItem>
            </Select>
            <div className="text-xs text-muted-foreground">
              Selected:{" "}
              {Math.floor(settings.roundTimeSeconds / 60)}:{(settings.roundTimeSeconds % 60)
                .toString().padStart(2, "0")}
            </div>
          </div>

          {/* Settings Preview */}
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm">Game Preview</CardTitle>
            </CardHeader>
            <CardContent className="pt-0">
              <div className="text-sm space-y-1">
                <div>• {settings.maxRounds} rounds total</div>
                <div>
                  • {Math.floor(settings.roundTimeSeconds / 60)}:{(settings.roundTimeSeconds % 60)
                    .toString().padStart(2, "0")} per round
                </div>
                <div>
                  • Estimated game time: ~{Math.ceil(
                    settings.maxRounds * (settings.roundTimeSeconds / 60) * 1.2,
                  )} minutes
                </div>
              </div>
            </CardContent>
          </Card>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={handleCancel}>
            Cancel
          </Button>
          <Button onClick={handleSave}>
            Save Settings
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
