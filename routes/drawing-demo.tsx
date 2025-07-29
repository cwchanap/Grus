import { Head } from "$fresh/runtime.ts";
import DrawingEngine from "../islands/DrawingEngine.tsx";
import { DrawingCommand } from "../types/game.ts";

export default function DrawingDemo() {
  const handleDrawingCommand = (command: DrawingCommand) => {
    console.log("Drawing command:", command);
  };

  const handleDrawingCommands = (commands: DrawingCommand[]) => {
    console.log("Drawing commands batch:", commands);
  };

  return (
    <>
      <Head>
        <title>Drawing Engine Demo</title>
      </Head>
      <div class="container mx-auto p-4">
        <h1 class="text-3xl font-bold mb-6">Drawing Engine Demo</h1>

        <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
          {/* Drawer View */}
          <div class="space-y-4">
            <h2 class="text-xl font-semibold">Drawer Mode</h2>
            <DrawingEngine
              isDrawer
              onDrawingCommand={handleDrawingCommand}
              onDrawingCommands={handleDrawingCommands}
              width={600}
              height={400}
            />
          </div>

          {/* Viewer Mode */}
          <div class="space-y-4">
            <h2 class="text-xl font-semibold">Viewer Mode</h2>
            <DrawingEngine
              isDrawer={false}
              onDrawingCommand={handleDrawingCommand}
              width={600}
              height={400}
            />
          </div>
        </div>

        {/* Disabled Mode */}
        <div class="mt-8 space-y-4">
          <h2 class="text-xl font-semibold">Disabled Mode</h2>
          <DrawingEngine
            isDrawer
            onDrawingCommand={handleDrawingCommand}
            width={600}
            height={300}
            disabled
          />
        </div>

        {/* Instructions */}
        <div class="mt-8 p-4 bg-gray-100 rounded-lg">
          <h3 class="text-lg font-semibold mb-2">Instructions</h3>
          <ul class="list-disc list-inside space-y-1 text-sm">
            <li>
              <strong>Drawer Mode:</strong>{" "}
              You can draw, change colors, adjust brush size, clear canvas, and undo
            </li>
            <li>
              <strong>Viewer Mode:</strong> You can only watch (no drawing tools available)
            </li>
            <li>
              <strong>Disabled Mode:</strong> Drawing is completely disabled
            </li>
            <li>Check the browser console to see drawing commands being generated</li>
            <li>Touch devices are supported for drawing</li>
          </ul>
        </div>
      </div>
    </>
  );
}
