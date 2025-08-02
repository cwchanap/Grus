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

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent onClose={onClose} className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Game Settings</DialogTitle>
          <DialogDescription>
            Configure the game rules and timing for your room.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
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
              Round Time
            </label>
            <div className="flex space-x-2">
              <div className="flex-1 space-y-1">
                <Select
                  value={settings.roundTimeMinutes.toString()}
                  onChange={(e) =>
                    setSettings((prev) => ({
                      ...prev,
                      roundTimeMinutes: parseInt((e.target as HTMLSelectElement).value),
                    }))}
                >
                  <SelectItem value="0">0</SelectItem>
                  <SelectItem value="1">1</SelectItem>
                  <SelectItem value="2">2</SelectItem>
                  <SelectItem value="3">3</SelectItem>
                  <SelectItem value="4">4</SelectItem>
                  <SelectItem value="5">5</SelectItem>
                </Select>
                <label className="text-xs text-muted-foreground">Minutes</label>
              </div>
              <div className="flex-1 space-y-1">
                <Select
                  value={settings.roundTimeSeconds.toString()}
                  onChange={(e) =>
                    setSettings((prev) => ({
                      ...prev,
                      roundTimeSeconds: parseInt((e.target as HTMLSelectElement).value),
                    }))}
                >
                  <SelectItem value="0">00</SelectItem>
                  <SelectItem value="15">15</SelectItem>
                  <SelectItem value="30">30</SelectItem>
                  <SelectItem value="45">45</SelectItem>
                </Select>
                <label className="text-xs text-muted-foreground">Seconds</label>
              </div>
            </div>
            <div className="text-xs text-muted-foreground">
              Total:{" "}
              {settings.roundTimeMinutes}:{settings.roundTimeSeconds.toString().padStart(2, "0")}
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
                  • {settings.roundTimeMinutes}:{settings.roundTimeSeconds.toString().padStart(
                    2,
                    "0",
                  )} per round
                </div>
                <div>
                  • Estimated game time: ~{Math.ceil(
                    settings.maxRounds *
                      (settings.roundTimeMinutes + settings.roundTimeSeconds / 60) * 1.5,
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
