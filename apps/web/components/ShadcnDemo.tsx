import { useState } from "preact/hooks";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Label,
  Select,
  SelectItem,
  Separator,
} from "./ui/index.ts";

export default function ShadcnDemo() {
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [selectedValue, setSelectedValue] = useState("");

  return (
    <div className="p-6 space-y-6 max-w-4xl mx-auto">
      <div className="space-y-2">
        <h1 className="text-3xl font-bold">shadcn/ui Components Demo</h1>
        <p className="text-muted-foreground">
          Showcasing the shadcn/ui components adapted for Deno Fresh + Preact
        </p>
      </div>

      <Separator />

      {/* Buttons */}
      <Card>
        <CardHeader>
          <CardTitle>Buttons</CardTitle>
          <CardDescription>Different button variants and sizes</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap gap-2">
            <Button>Default</Button>
            <Button variant="secondary">Secondary</Button>
            <Button variant="outline">Outline</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="link">Link</Button>
            <Button variant="destructive">Destructive</Button>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button size="sm">Small</Button>
            <Button size="default">Default</Button>
            <Button size="lg">Large</Button>
            <Button size="icon">🎨</Button>
          </div>
        </CardContent>
      </Card>

      {/* Badges */}
      <Card>
        <CardHeader>
          <CardTitle>Badges</CardTitle>
          <CardDescription>Status indicators and labels</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex flex-wrap gap-2">
            <Badge>Default</Badge>
            <Badge variant="secondary">Secondary</Badge>
            <Badge variant="outline">Outline</Badge>
            <Badge variant="destructive">Destructive</Badge>
          </div>
        </CardContent>
      </Card>

      {/* Form Elements */}
      <Card>
        <CardHeader>
          <CardTitle>Form Elements</CardTitle>
          <CardDescription>Inputs, labels, and selects</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="email">Email</Label>
            <Input
              id="email"
              type="email"
              placeholder="Enter your email"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="select">Select Option</Label>
            <Select
              id="select"
              value={selectedValue}
              onChange={(e) => setSelectedValue((e.target as HTMLSelectElement).value)}
            >
              <SelectItem value="">Choose an option</SelectItem>
              <SelectItem value="option1">Option 1</SelectItem>
              <SelectItem value="option2">Option 2</SelectItem>
              <SelectItem value="option3">Option 3</SelectItem>
            </Select>
          </div>
        </CardContent>
      </Card>

      {/* Dialog */}
      <Card>
        <CardHeader>
          <CardTitle>Dialog</CardTitle>
          <CardDescription>Modal dialogs and overlays</CardDescription>
        </CardHeader>
        <CardContent>
          <Button onClick={() => setIsDialogOpen(true)}>
            Open Dialog
          </Button>
        </CardContent>
      </Card>

      <Dialog open={isDialogOpen} onOpenChange={setIsDialogOpen}>
        <DialogContent onClose={() => setIsDialogOpen(false)}>
          <DialogHeader>
            <DialogTitle>Example Dialog</DialogTitle>
            <DialogDescription>
              This is an example dialog using the shadcn/ui components adapted for Preact.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="dialog-input">Name</Label>
              <Input
                id="dialog-input"
                placeholder="Enter your name"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setIsDialogOpen(false)}>
              Cancel
            </Button>
            <Button onClick={() => setIsDialogOpen(false)}>
              Save
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Nested Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              Game Stats
              <Badge variant="secondary">Live</Badge>
            </CardTitle>
            <CardDescription>Current game statistics</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              <div className="flex justify-between">
                <span>Players:</span>
                <Badge>4/8</Badge>
              </div>
              <div className="flex justify-between">
                <span>Round:</span>
                <Badge variant="outline">2/5</Badge>
              </div>
              <div className="flex justify-between">
                <span>Status:</span>
                <Badge variant="destructive">Drawing</Badge>
              </div>
            </div>
          </CardContent>
          <CardFooter>
            <Button className="w-full" variant="outline">
              View Details
            </Button>
          </CardFooter>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Quick Actions</CardTitle>
            <CardDescription>Common game actions</CardDescription>
          </CardHeader>
          <CardContent className="space-y-2">
            <Button className="w-full" variant="outline">
              🎨 Start Drawing
            </Button>
            <Button className="w-full" variant="outline">
              💬 Open Chat
            </Button>
            <Button className="w-full" variant="outline">
              ⚙️ Game Settings
            </Button>
          </CardContent>
          <CardFooter>
            <Button className="w-full" variant="destructive">
              Leave Game
            </Button>
          </CardFooter>
        </Card>
      </div>
    </div>
  );
}
