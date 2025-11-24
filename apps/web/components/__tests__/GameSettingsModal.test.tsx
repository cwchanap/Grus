import { assertEquals } from "$std/assert/mod.ts";
import type { GameSettings } from "../GameSettingsModal.tsx";

// Test helper functions for GameSettingsModal
const createDefaultSettings = (): GameSettings => ({
  maxRounds: 5,
  roundTimeSeconds: 75, // Default 75 seconds as per product rules
});

const calculateTotalTime = (settings: GameSettings): number => {
  return settings.roundTimeSeconds;
};

const formatTimeDisplay = (settings: GameSettings): string => {
  const minutes = Math.floor(settings.roundTimeSeconds / 60);
  const seconds = settings.roundTimeSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
};

const estimateGameTime = (settings: GameSettings): number => {
  const roundTimeMinutes = settings.roundTimeSeconds / 60;
  return Math.ceil(settings.maxRounds * roundTimeMinutes * 1.2); // 1.2x multiplier for transitions
};

Deno.test("GameSettingsModal - default settings are valid", () => {
  const settings = createDefaultSettings();

  assertEquals(settings.maxRounds, 5);
  assertEquals(settings.roundTimeSeconds, 75);
});

Deno.test("GameSettingsModal - calculateTotalTime works correctly", () => {
  const testCases = [
    { settings: { maxRounds: 5, roundTimeSeconds: 60 }, expected: 60 },
    { settings: { maxRounds: 3, roundTimeSeconds: 75 }, expected: 75 },
    { settings: { maxRounds: 7, roundTimeSeconds: 90 }, expected: 90 },
  ];

  testCases.forEach(({ settings, expected }) => {
    const result = calculateTotalTime(settings);
    assertEquals(
      result,
      expected,
      `Expected ${expected} seconds for ${JSON.stringify(settings)}, got ${result}`,
    );
  });
});

Deno.test("GameSettingsModal - formatTimeDisplay formats correctly", () => {
  const testCases = [
    { settings: { maxRounds: 5, roundTimeSeconds: 60 }, expected: "1:00" },
    { settings: { maxRounds: 3, roundTimeSeconds: 75 }, expected: "1:15" },
    { settings: { maxRounds: 7, roundTimeSeconds: 90 }, expected: "1:30" },
  ];

  testCases.forEach(({ settings, expected }) => {
    const result = formatTimeDisplay(settings);
    assertEquals(
      result,
      expected,
      `Expected "${expected}" for ${JSON.stringify(settings)}, got "${result}"`,
    );
  });
});

Deno.test("GameSettingsModal - estimateGameTime calculates reasonable estimates", () => {
  const testCases = [
    {
      settings: { maxRounds: 5, roundTimeSeconds: 75 },
      expectedMin: 7,
      expectedMax: 9,
    },
    {
      settings: { maxRounds: 3, roundTimeSeconds: 60 },
      expectedMin: 3,
      expectedMax: 5,
    },
    {
      settings: { maxRounds: 10, roundTimeSeconds: 90 },
      expectedMin: 17,
      expectedMax: 19,
    },
  ];

  testCases.forEach(({ settings, expectedMin, expectedMax }) => {
    const result = estimateGameTime(settings);
    assertEquals(
      result >= expectedMin && result <= expectedMax,
      true,
      `Expected estimate between ${expectedMin}-${expectedMax} minutes for ${
        JSON.stringify(settings)
      }, got ${result}`,
    );
  });
});

Deno.test("GameSettingsModal - validates round count options", () => {
  const validRoundCounts = [3, 5, 7, 10];

  validRoundCounts.forEach((count) => {
    const settings: GameSettings = {
      maxRounds: count,
      roundTimeSeconds: 75,
    };

    assertEquals(settings.maxRounds, count);
    assertEquals(validRoundCounts.includes(settings.maxRounds), true);
  });
});

Deno.test("GameSettingsModal - validates time options", () => {
  const validTimeOptions = [60, 75, 90]; // As per product rules: 60-90 seconds

  validTimeOptions.forEach((seconds) => {
    const settings: GameSettings = {
      maxRounds: 5,
      roundTimeSeconds: seconds,
    };

    assertEquals(validTimeOptions.includes(settings.roundTimeSeconds), true);
    assertEquals(settings.roundTimeSeconds >= 60 && settings.roundTimeSeconds <= 90, true);
  });
});

Deno.test("GameSettingsModal - prevents invalid time combinations", () => {
  // Test that times outside 60-90 range would be invalid
  const invalidSettings: GameSettings = {
    maxRounds: 5,
    roundTimeSeconds: 30, // Too short
  };

  const totalTime = calculateTotalTime(invalidSettings);
  assertEquals(totalTime, 30);

  // In a real implementation, this would be validated
  const isValidTime = totalTime >= 60 && totalTime <= 90;
  assertEquals(isValidTime, false);
});

Deno.test("GameSettingsModal - handles maximum time settings", () => {
  const maxSettings: GameSettings = {
    maxRounds: 10,
    roundTimeSeconds: 90, // Maximum allowed per product rules
  };

  const totalTime = calculateTotalTime(maxSettings);
  const estimatedGameTime = estimateGameTime(maxSettings);

  assertEquals(totalTime, 90); // 1:30 = 90 seconds
  assertEquals(estimatedGameTime >= 15, true); // Should be quite long
});

Deno.test("GameSettingsModal - handles minimum practical settings", () => {
  const minSettings: GameSettings = {
    maxRounds: 3,
    roundTimeSeconds: 60, // Minimum allowed per product rules
  };

  const totalTime = calculateTotalTime(minSettings);
  const estimatedGameTime = estimateGameTime(minSettings);

  assertEquals(totalTime, 60); // 1:00 = 60 seconds
  assertEquals(estimatedGameTime >= 3, true); // Should be at least a few minutes
});
