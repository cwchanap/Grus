import { useEffect, useState } from "preact/hooks";
import { TouchButton, useIsMobile } from "./MobileOptimized.tsx";

export interface DrawingTool {
  color: string;
  size: number;
  type: "brush";
}

interface MobileDrawingToolsProps {
  currentTool: DrawingTool;
  onToolChange: (tool: DrawingTool) => void;
  onClear: () => void;
  onUndo: () => void;
  canUndo: boolean;
  disabled?: boolean;
}

const QUICK_COLORS = [
  "#000000", // Black
  "#FF0000", // Red
  "#00FF00", // Green
  "#0000FF", // Blue
  "#FFFF00", // Yellow
  "#FF00FF", // Magenta
  "#00FFFF", // Cyan
  "#FFA500", // Orange
  "#800080", // Purple
  "#FFC0CB", // Pink
  "#A52A2A", // Brown
  "#808080", // Gray
];

const QUICK_SIZES = [2, 5, 10, 15, 20];

export default function MobileDrawingTools({
  currentTool,
  onToolChange,
  onClear,
  onUndo,
  canUndo,
  disabled = false,
}: MobileDrawingToolsProps) {
  const { isMobile, isTouch: _isTouch } = useIsMobile();
  const [showColorPicker, setShowColorPicker] = useState(false);
  const [showSizePicker, setShowSizePicker] = useState(false);

  // Close pickers when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: Event) => {
      const target = e.target as HTMLElement;
      if (!target.closest(".color-picker-container") && !target.closest(".color-picker-button")) {
        setShowColorPicker(false);
      }
      if (!target.closest(".size-picker-container") && !target.closest(".size-picker-button")) {
        setShowSizePicker(false);
      }
    };

    document.addEventListener("click", handleClickOutside);
    return () => document.removeEventListener("click", handleClickOutside);
  }, []);

  const handleColorChange = (color: string) => {
    onToolChange({ ...currentTool, color });
    setShowColorPicker(false);
  };

  const handleSizeChange = (size: number) => {
    onToolChange({ ...currentTool, size });
    setShowSizePicker(false);
  };

  if (disabled) {
    return (
      <div class="drawing-tools-disabled p-3 bg-gray-100 rounded-lg text-center">
        <span class="text-gray-500 text-sm">Drawing tools disabled</span>
      </div>
    );
  }

  return (
    <div class="mobile-drawing-tools bg-gray-50 rounded-lg p-3 border">
      {/* Mobile Layout */}
      {isMobile
        ? (
          <div class="space-y-3">
            {/* Top row: Color and Size selectors */}
            <div class="flex items-center gap-3">
              {/* Color Picker */}
              <div class="relative">
                <button
                  type="button"
                  onClick={() => setShowColorPicker(!showColorPicker)}
                  class="color-picker-button flex items-center gap-2 px-3 py-2 bg-white border border-gray-300 rounded-lg touch-manipulation no-tap-highlight"
                >
                  <div
                    class="w-6 h-6 rounded border-2 border-gray-400"
                    style={{ backgroundColor: currentTool.color }}
                  />
                  <span class="text-sm font-medium">Color</span>
                  <span class="text-xs text-gray-500">▼</span>
                </button>

                {/* Color Picker Dropdown */}
                {showColorPicker && (
                  <div class="color-picker-container absolute top-full left-0 mt-1 bg-white border border-gray-300 rounded-lg shadow-lg p-3 z-50 min-w-[200px]">
                    <div class="grid grid-cols-4 gap-2 mb-3">
                      {QUICK_COLORS.map((color) => (
                        <button
                          type="button"
                          key={color}
                          onClick={() => handleColorChange(color)}
                          class={`w-10 h-10 rounded border-2 touch-manipulation no-tap-highlight ${
                            currentTool.color === color ? "border-gray-800" : "border-gray-300"
                          }`}
                          style={{ backgroundColor: color }}
                          title={color}
                        />
                      ))}
                    </div>
                    <div class="border-t pt-2">
                      <label class="block text-xs text-gray-600 mb-1">Custom Color:</label>
                      <input
                        type="color"
                        value={currentTool.color}
                        onChange={(e) => handleColorChange((e.target as HTMLInputElement).value)}
                        class="w-full h-10 rounded border cursor-pointer"
                      />
                    </div>
                  </div>
                )}
              </div>

              {/* Size Picker */}
              <div class="relative">
                <button
                  type="button"
                  onClick={() => setShowSizePicker(!showSizePicker)}
                  class="size-picker-button flex items-center gap-2 px-3 py-2 bg-white border border-gray-300 rounded-lg touch-manipulation no-tap-highlight"
                >
                  <div class="flex items-center justify-center w-6 h-6">
                    <div
                      class="bg-gray-800 rounded-full"
                      style={{
                        width: `${Math.min(currentTool.size, 20)}px`,
                        height: `${Math.min(currentTool.size, 20)}px`,
                      }}
                    />
                  </div>
                  <span class="text-sm font-medium">{currentTool.size}px</span>
                  <span class="text-xs text-gray-500">▼</span>
                </button>

                {/* Size Picker Dropdown */}
                {showSizePicker && (
                  <div class="size-picker-container absolute top-full left-0 mt-1 bg-white border border-gray-300 rounded-lg shadow-lg p-3 z-50 min-w-[160px]">
                    <div class="space-y-2 mb-3">
                      {QUICK_SIZES.map((size) => (
                        <button
                          type="button"
                          key={size}
                          onClick={() => handleSizeChange(size)}
                          class={`w-full flex items-center gap-3 px-2 py-2 rounded touch-manipulation no-tap-highlight ${
                            currentTool.size === size
                              ? "bg-blue-100 border border-blue-300"
                              : "hover:bg-gray-100"
                          }`}
                        >
                          <div class="flex items-center justify-center w-8 h-8">
                            <div
                              class="bg-gray-800 rounded-full"
                              style={{
                                width: `${Math.min(size, 24)}px`,
                                height: `${Math.min(size, 24)}px`,
                              }}
                            />
                          </div>
                          <span class="text-sm">{size}px</span>
                        </button>
                      ))}
                    </div>
                    <div class="border-t pt-2">
                      <label class="block text-xs text-gray-600 mb-1">Custom Size:</label>
                      <input
                        type="range"
                        min="1"
                        max="30"
                        value={currentTool.size}
                        onChange={(e) =>
                          handleSizeChange(parseInt((e.target as HTMLInputElement).value))}
                        class="w-full h-8 touch-manipulation"
                      />
                    </div>
                  </div>
                )}
              </div>
            </div>

            {/* Bottom row: Action buttons */}
            <div class="flex gap-2">
              <TouchButton
                onClick={onClear}
                variant="danger"
                size="md"
                className="flex-1"
              >
                Clear All
              </TouchButton>
              <TouchButton
                onClick={onUndo}
                variant="secondary"
                size="md"
                disabled={!canUndo}
                className="flex-1"
              >
                Undo
              </TouchButton>
            </div>
          </div>
        )
        : (
          /* Desktop Layout */
          <div class="flex items-center gap-4 flex-wrap">
            {/* Color Picker */}
            <div class="flex items-center gap-2">
              <label class="text-sm font-medium">Color:</label>
              <input
                type="color"
                value={currentTool.color}
                onChange={(e) =>
                  onToolChange({
                    ...currentTool,
                    color: (e.target as HTMLInputElement).value,
                  })}
                class="w-8 h-8 rounded border cursor-pointer"
              />
              {/* Quick colors */}
              <div class="flex gap-1 ml-2">
                {QUICK_COLORS.slice(0, 6).map((color) => (
                  <button
                    type="button"
                    key={color}
                    onClick={() => onToolChange({ ...currentTool, color })}
                    class={`w-6 h-6 rounded border-2 ${
                      currentTool.color === color ? "border-gray-800" : "border-gray-300"
                    }`}
                    style={{ backgroundColor: color }}
                  />
                ))}
              </div>
            </div>

            {/* Size Picker */}
            <div class="flex items-center gap-2 flex-1 min-w-0">
              <label class="text-sm font-medium">Size:</label>
              <input
                type="range"
                min="1"
                max="30"
                value={currentTool.size}
                onChange={(e) =>
                  onToolChange({
                    ...currentTool,
                    size: parseInt((e.target as HTMLInputElement).value),
                  })}
                class="flex-1 min-w-0"
              />
              <span class="text-sm w-8 text-center">{currentTool.size}</span>
            </div>

            {/* Action Buttons */}
            <div class="flex gap-2">
              <button
                type="button"
                onClick={onClear}
                class="px-3 py-1 bg-red-500 text-white rounded hover:bg-red-600 text-sm font-medium"
              >
                Clear
              </button>
              <button
                type="button"
                onClick={onUndo}
                disabled={!canUndo}
                class="px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:bg-gray-400 text-sm font-medium"
              >
                Undo
              </button>
            </div>
          </div>
        )}
    </div>
  );
}
