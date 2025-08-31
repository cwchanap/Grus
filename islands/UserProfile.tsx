import { useState } from "preact/hooks";
import { Button } from "../components/ui/button.tsx";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "../components/ui/card.tsx";
import { Input } from "../components/ui/input.tsx";
import { Label } from "../components/ui/label.tsx";
import { ArrowLeft, Key, Mail, User, Users } from "lucide-react";
import type { UserPayload } from "../lib/auth/auth-utils.ts";

interface UserProfileProps {
  user: UserPayload;
}

export default function UserProfile({ user }: UserProfileProps) {
  const [passwordData, setPasswordData] = useState({
    currentPassword: "",
    newPassword: "",
    confirmPassword: "",
  });
  const [displayName, setDisplayName] = useState(user.name || "");
  const [isEditingName, setIsEditingName] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const [loading, setLoading] = useState(false);
  const [nameLoading, setNameLoading] = useState(false);

  const handlePasswordChange = async (e: Event) => {
    e.preventDefault();
    setError("");
    setSuccess("");

    // Validate passwords match
    if (passwordData.newPassword !== passwordData.confirmPassword) {
      setError("New passwords do not match");
      return;
    }

    // Validate password length
    if (passwordData.newPassword.length < 6) {
      setError("New password must be at least 6 characters");
      return;
    }

    setLoading(true);

    try {
      const response = await fetch("/api/auth/change-password", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          currentPassword: passwordData.currentPassword,
          newPassword: passwordData.newPassword,
        }),
      });

      const data = await response.json();

      if (!response.ok) {
        setError(data.error || "Failed to change password");
        return;
      }

      setSuccess("Password changed successfully!");
      setPasswordData({
        currentPassword: "",
        newPassword: "",
        confirmPassword: "",
      });
    } catch (_err) {
      setError("An error occurred. Please try again.");
    } finally {
      setLoading(false);
    }
  };

  const handlePasswordInputChange = (e: Event) => {
    const target = e.target as HTMLInputElement;
    setPasswordData({
      ...passwordData,
      [target.name]: target.value,
    });
  };

  const handleDisplayNameUpdate = async () => {
    setError("");
    setSuccess("");
    setNameLoading(true);

    try {
      const response = await fetch("/api/auth/update-profile", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          name: displayName.trim(),
        }),
      });

      const data = await response.json();

      if (!response.ok) {
        setError(data.error || "Failed to update display name");
        return;
      }

      setSuccess("Display name updated successfully!");
      setIsEditingName(false);

      // Update the user object in the URL or reload to reflect changes
      setTimeout(() => {
        globalThis.location.reload();
      }, 1000);
    } catch (_err) {
      setError("An error occurred. Please try again.");
    } finally {
      setNameLoading(false);
    }
  };

  const handleCancelNameEdit = () => {
    setDisplayName(user.name || "");
    setIsEditingName(false);
    setError("");
    setSuccess("");
  };

  const generateAvatarUrl = (username: string) => {
    // Generate a simple avatar using the first letter and background color based on username
    const colors = [
      "bg-red-500",
      "bg-blue-500",
      "bg-green-500",
      "bg-yellow-500",
      "bg-purple-500",
      "bg-pink-500",
      "bg-indigo-500",
      "bg-gray-500",
    ];
    const colorIndex = username.charCodeAt(0) % colors.length;
    return colors[colorIndex];
  };

  return (
    <div className="max-w-2xl mx-auto">
      {/* Header with Back Button */}
      <div className="mb-6">
        <Button
          variant="ghost"
          size="sm"
          className="text-white hover:bg-white/20 mb-4"
          onClick={() => globalThis.history.back()}
        >
          <ArrowLeft className="w-4 h-4 mr-2" />
          Back
        </Button>
        <h1 className="text-2xl md:text-3xl font-bold text-white">User Profile</h1>
      </div>

      <div className="space-y-6">
        {/* Profile Information Card */}
        <Card className="bg-white/95 backdrop-blur-sm">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <User className="w-5 h-5" />
              Profile Information
            </CardTitle>
            <CardDescription>
              Your account details and information
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            {/* Avatar Section */}
            <div className="flex flex-col sm:flex-row items-center sm:items-start gap-4">
              <div
                className={`w-20 h-20 rounded-full ${
                  generateAvatarUrl(user.username)
                } flex items-center justify-center text-white text-2xl font-bold flex-shrink-0`}
              >
                {(user.name || user.username).charAt(0).toUpperCase()}
              </div>
              <div className="text-center sm:text-left">
                <h3 className="text-lg font-semibold text-gray-800">
                  {user.name || user.username}
                </h3>
                <p className="text-gray-600">@{user.username}</p>
              </div>
            </div>

            {/* User Details */}
            <div className="grid gap-4">
              <div className="space-y-2">
                <Label className="flex items-center gap-2">
                  <Mail className="w-4 h-4" />
                  Email
                </Label>
                <Input
                  value={user.email}
                  disabled
                  className="bg-gray-50"
                />
              </div>

              <div className="space-y-2">
                <Label className="flex items-center gap-2">
                  <User className="w-4 h-4" />
                  Username
                </Label>
                <Input
                  value={user.username}
                  disabled
                  className="bg-gray-50"
                />
              </div>

              <div className="space-y-2">
                <Label className="flex items-center gap-2">
                  <Users className="w-4 h-4" />
                  Display Name
                </Label>
                {isEditingName
                  ? (
                    <div className="space-y-2">
                      <Input
                        value={displayName}
                        onInput={(e) => setDisplayName((e.target as HTMLInputElement).value)}
                        placeholder="Enter display name"
                        maxLength={100}
                        disabled={nameLoading}
                      />
                      <div className="flex gap-2">
                        <Button
                          type="button"
                          size="sm"
                          onClick={handleDisplayNameUpdate}
                          disabled={nameLoading}
                        >
                          {nameLoading ? "Saving..." : "Save"}
                        </Button>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={handleCancelNameEdit}
                          disabled={nameLoading}
                        >
                          Cancel
                        </Button>
                      </div>
                    </div>
                  )
                  : (
                    <div className="flex items-center gap-2">
                      <Input
                        value={user.name || "Not set"}
                        disabled
                        className="bg-gray-50 flex-1"
                      />
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => setIsEditingName(true)}
                      >
                        Edit
                      </Button>
                    </div>
                  )}
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Change Password Card */}
        <Card className="bg-white/95 backdrop-blur-sm">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Key className="w-5 h-5" />
              Change Password
            </CardTitle>
            <CardDescription>
              Update your account password for better security
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={handlePasswordChange} className="space-y-4">
              {error && (
                <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
                  {error}
                </div>
              )}

              {success && (
                <div className="bg-green-50 border border-green-200 text-green-700 px-4 py-3 rounded">
                  {success}
                </div>
              )}

              <div className="space-y-2">
                <Label htmlFor="currentPassword">Current Password</Label>
                <Input
                  id="currentPassword"
                  name="currentPassword"
                  type="password"
                  placeholder="Enter your current password"
                  value={passwordData.currentPassword}
                  onInput={handlePasswordInputChange}
                  required
                  disabled={loading}
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="newPassword">New Password</Label>
                <Input
                  id="newPassword"
                  name="newPassword"
                  type="password"
                  placeholder="Enter your new password"
                  value={passwordData.newPassword}
                  onInput={handlePasswordInputChange}
                  required
                  disabled={loading}
                />
                <p className="text-sm text-gray-500">
                  Password must be at least 6 characters long
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="confirmPassword">Confirm New Password</Label>
                <Input
                  id="confirmPassword"
                  name="confirmPassword"
                  type="password"
                  placeholder="Confirm your new password"
                  value={passwordData.confirmPassword}
                  onInput={handlePasswordInputChange}
                  required
                  disabled={loading}
                />
              </div>

              <Button
                type="submit"
                className="w-full"
                disabled={loading}
              >
                {loading ? "Changing Password..." : "Change Password"}
              </Button>
            </form>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
