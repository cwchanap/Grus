import { useEffect, useRef, useState } from "preact/hooks";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "../components/ui/dialog.tsx";
import { Button } from "../components/ui/button.tsx";

interface AvatarModalProps {
  open: boolean;
  onClose: () => void;
  onSaved?: () => void;
}

// Simple image cropper with drag + zoom and 64x64 export
export default function AvatarModal({ open, onClose, onSaved }: AvatarModalProps) {
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const viewCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const previewCanvasRef = useRef<HTMLCanvasElement | null>(null);

  const [image, setImage] = useState<HTMLImageElement | null>(null);
  const [scale, setScale] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const [lastPos, setLastPos] = useState<{ x: number; y: number } | null>(null);
  const [error, setError] = useState<string>("");

  const VIEW_SIZE = 256; // square viewport
  const PREVIEW_SIZE = 64; // final export size

  useEffect(() => {
    // Initialize canvas size
    const view = viewCanvasRef.current;
    const prev = previewCanvasRef.current;
    if (view) {
      view.width = VIEW_SIZE;
      view.height = VIEW_SIZE;
    }
    if (prev) {
      prev.width = PREVIEW_SIZE;
      prev.height = PREVIEW_SIZE;
    }
    draw();
  }, [image, scale, offset, open]);

  const draw = () => {
    const view = viewCanvasRef.current;
    if (!view) return;
    const ctx = view.getContext("2d");
    if (!ctx) return;

    // Clear
    ctx.clearRect(0, 0, VIEW_SIZE, VIEW_SIZE);
    ctx.fillStyle = "#f3f4f6"; // gray-100 background
    ctx.fillRect(0, 0, VIEW_SIZE, VIEW_SIZE);

    if (image) {
      ctx.save();
      ctx.setTransform(scale, 0, 0, scale, offset.x, offset.y);
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(image, 0, 0);
      ctx.restore();
    } else {
      // helper text
      ctx.fillStyle = "#9ca3af"; // gray-400
      ctx.font = "14px sans-serif";
      ctx.textAlign = "center";
      ctx.fillText("Click or drop an image", VIEW_SIZE / 2, VIEW_SIZE / 2);
    }

    // Draw preview
    const prev = previewCanvasRef.current;
    if (prev) {
      const pctx = prev.getContext("2d");
      if (!pctx) return;
      pctx.clearRect(0, 0, PREVIEW_SIZE, PREVIEW_SIZE);
      pctx.fillStyle = "#ffffff";
      pctx.fillRect(0, 0, PREVIEW_SIZE, PREVIEW_SIZE);
      if (image) {
        // We scale/translate proportionally from the view canvas space to 64px
        const scaleRatio = PREVIEW_SIZE / VIEW_SIZE; // 64/256 = 0.25
        pctx.save();
        pctx.setTransform(scale * scaleRatio, 0, 0, scale * scaleRatio, offset.x * scaleRatio, offset.y * scaleRatio);
        pctx.imageSmoothingQuality = "high";
        pctx.drawImage(image, 0, 0);
        pctx.restore();
      }
    }
  };

  const handleFileChange = (e: Event) => {
    const input = e.target as HTMLInputElement;
    if (!input.files || input.files.length === 0) return;
    const file = input.files[0];
    if (!file.type.startsWith("image/")) {
      setError("Please select an image file.");
      return;
    }
    const img = new Image();
    img.onload = () => {
      setImage(img);
      // Center the image in the viewport
      const initialScale = Math.min(VIEW_SIZE / img.width, VIEW_SIZE / img.height);
      setScale(initialScale);
      setOffset({ x: (VIEW_SIZE - img.width * initialScale) / 2, y: (VIEW_SIZE - img.height * initialScale) / 2 });
      setError("");
    };
    img.onerror = () => setError("Failed to load image.");
    const reader = new FileReader();
    reader.onload = () => {
      try {
        img.src = String(reader.result);
      } catch (_e) {
        setError("Could not read image file.");
      }
    };
    reader.onerror = () => setError("Failed to read image file.");
    reader.readAsDataURL(file);
  };

  const clampScale = (s: number) => Math.max(0.3, Math.min(5, s));

  const handleWheel = (e: WheelEvent) => {
    if (!image) return;
    e.preventDefault();
    const view = viewCanvasRef.current;
    if (!view) return;
    const rect = view.getBoundingClientRect();
    const cx = (e.clientX - rect.left);
    const cy = (e.clientY - rect.top);

    const zoomFactor = e.deltaY < 0 ? 1.1 : 0.9;
    const newScale = clampScale(scale * zoomFactor);

    // Zoom towards cursor: adjust offset so point under cursor stays under cursor
    const dx = cx - offset.x;
    const dy = cy - offset.y;
    const newOffset = {
      x: cx - (dx * newScale / scale),
      y: cy - (dy * newScale / scale),
    };

    setScale(newScale);
    setOffset(newOffset);
  };

  const startDrag = (e: MouseEvent | TouchEvent) => {
    setDragging(true);
    const { x, y } = getEventPos(e);
    setLastPos({ x, y });
  };

  const onDrag = (e: MouseEvent | TouchEvent) => {
    if (!dragging || !lastPos) return;
    const { x, y } = getEventPos(e);
    const dx = x - lastPos.x;
    const dy = y - lastPos.y;
    setOffset((o) => ({ x: o.x + dx, y: o.y + dy }));
    setLastPos({ x, y });
  };

  const endDrag = () => {
    setDragging(false);
    setLastPos(null);
  };

  const getEventPos = (e: MouseEvent | TouchEvent) => {
    const view = viewCanvasRef.current!;
    const rect = view.getBoundingClientRect();
    if (e instanceof MouseEvent) {
      return { x: e.clientX - rect.left, y: e.clientY - rect.top };
    } else {
      const t = e.touches[0];
      return { x: t.clientX - rect.left, y: t.clientY - rect.top };
    }
  };

  const handleSave = async () => {
    if (!image) return;
    // Render final 64x64
    const out = document.createElement("canvas");
    out.width = PREVIEW_SIZE;
    out.height = PREVIEW_SIZE;
    const ctx = out.getContext("2d");
    if (!ctx) return;
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, PREVIEW_SIZE, PREVIEW_SIZE);

    const scaleRatio = PREVIEW_SIZE / VIEW_SIZE; // 0.25
    ctx.setTransform(scale * scaleRatio, 0, 0, scale * scaleRatio, offset.x * scaleRatio, offset.y * scaleRatio);
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(image, 0, 0);

    const dataUrl = out.toDataURL("image/png");

    try {
      const resp = await fetch("/api/auth/update-avatar", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ avatar: dataUrl }),
      });
      const data = await resp.json().catch(() => ({}));
      if (!resp.ok) {
        setError(data.error || "Failed to update avatar");
        return;
      }
      onSaved?.();
      onClose();
      // Refresh to reflect in UI
      globalThis.location?.reload?.();
    } catch (err) {
      console.error(err);
      setError("Network error. Please try again.");
    }
  };

  const handleRemove = async () => {
    try {
      const resp = await fetch("/api/auth/update-avatar", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ avatar: "" }),
      });
      const data = await resp.json().catch(() => ({}));
      if (!resp.ok) {
        setError(data.error || "Failed to remove avatar");
        return;
      }
      onSaved?.();
      onClose();
      globalThis.location?.reload?.();
    } catch (err) {
      console.error(err);
      setError("Network error. Please try again.");
    }
  };

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    if (!e.dataTransfer || !e.dataTransfer.files?.length) return;
    const file = e.dataTransfer.files[0];
    if (!file.type.startsWith("image/")) {
      setError("Please drop an image file.");
      return;
    }
    const img = new Image();
    img.onload = () => {
      setImage(img);
      const initialScale = Math.min(VIEW_SIZE / img.width, VIEW_SIZE / img.height);
      setScale(initialScale);
      setOffset({ x: (VIEW_SIZE - img.width * initialScale) / 2, y: (VIEW_SIZE - img.height * initialScale) / 2 });
      setError("");
    };
    img.onerror = () => setError("Failed to load image.");
    const reader = new FileReader();
    reader.onload = () => {
      try {
        img.src = String(reader.result);
      } catch (_e) {
        setError("Could not read dropped image file.");
      }
    };
    reader.onerror = () => setError("Failed to read dropped image file.");
    reader.readAsDataURL(file);
  };

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault();
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl" onClose={onClose} data-testid="avatar-modal">
        <DialogHeader>
          <DialogTitle>Update Avatar</DialogTitle>
          <DialogDescription>Upload an image, then drag and zoom to crop. The result will be saved at 64x64.</DialogDescription>
        </DialogHeader>

        <div className="p-6 pt-0 grid grid-cols-1 md:grid-cols-2 gap-6">
          <div>
            <div
              className="w-[256px] h-[256px] border rounded-md bg-gray-100 relative cursor-move select-none"
              onWheel={(e) => handleWheel(e as unknown as WheelEvent)}
              onMouseDown={(e) => startDrag(e as unknown as MouseEvent)}
              onMouseMove={(e) => onDrag(e as unknown as MouseEvent)}
              onMouseUp={endDrag}
              onMouseLeave={endDrag}
              onTouchStart={(e) => startDrag(e as unknown as TouchEvent)}
              onTouchMove={(e) => onDrag(e as unknown as TouchEvent)}
              onTouchEnd={endDrag}
              onDrop={(e) => handleDrop(e as unknown as DragEvent)}
              onDragOver={(e) => handleDragOver(e as unknown as DragEvent)}
            >
              <canvas ref={viewCanvasRef} className="absolute inset-0" width={VIEW_SIZE} height={VIEW_SIZE} data-testid="avatar-view-canvas" />
            </div>
            <div className="mt-4 flex items-center gap-3">
              <input
                ref={fileInputRef}
                type="file"
                accept="image/*"
                onChange={handleFileChange}
                className="block w-full text-sm text-gray-900 border border-gray-300 rounded-lg cursor-pointer bg-gray-50 focus:outline-none"
                data-testid="avatar-file-input"
              />
            </div>
            <div className="mt-3 flex items-center gap-2">
              <label className="text-sm text-gray-600">Zoom</label>
              <input
                type="range"
                min="0.3"
                max="5"
                step="0.01"
                value={scale}
                onInput={(e) => setScale(parseFloat((e.target as HTMLInputElement).value))}
                data-testid="avatar-zoom-range"
              />
            </div>
            {error && (
              <div className="mt-3 text-sm text-red-600">{error}</div>
            )}
          </div>

          <div>
            <div className="text-sm text-gray-700 mb-2">Preview (64x64)</div>
            <div className="w-[64px] h-[64px] overflow-hidden rounded-full border">
              <canvas ref={previewCanvasRef} width={PREVIEW_SIZE} height={PREVIEW_SIZE} data-testid="avatar-preview-canvas" />
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose} data-testid="avatar-cancel">Cancel</Button>
          <Button variant="outline" onClick={handleRemove} data-testid="avatar-remove">Remove Avatar</Button>
          <Button onClick={handleSave} data-testid="avatar-save">Save Avatar</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
