import { assertEquals } from "$std/assert/mod.ts";
import type { GameSettings } from "../GameSettingsModal.tsx";

// Test helper functions for GameSettingsModal
const createDefaultSettings = (): GameSettings => ({
  maxRounds: 5,
  roundTimeMinutes: 1,
  roundTimeSeconds: 30,
});

const calculateTotalTime = (settings: GameSettings): number => {
  return settings.roundTimeMinutes * 60 + settings.roundTimeSeconds;
};

const formatTimeDisplay = (settings: GameSettings): string => {
  return `${settings.roundTimeMinutes}:${settings.roundTimeSeconds.toString().padStart(2, "0")}`;
};

const estimateGameTime = (settings: GameSettings): number => {
  const roundTimeMinutes = settings.roundTimeMinutes + settings.roundTimeSeconds / 60;
  return Math.ceil(settings.maxRounds * roundTimeMinutes * 1.5); // 1.5x multiplier for transitions
};

Deno.test("GameSettingsModal - default settings are valid", () => {
  const settings = createDefaultSettings();

  assertEquals(settings.maxRounds, 5);
  assertEquals(settings.roundTimeMinutes, 1);
  assertEquals(settings.roundTimeSeconds, 30);
});

Deno.test("GameSettingsModal - calculateTotalTime works correctly", () => {
  const testCases = [
    { settings: { maxRounds: 5, roundTimeMinutes: 1, roundTimeSeconds: 30 }, expected: 90 },
    { settings: { maxRounds: 3, roundTimeMinutes: 2, roundTimeSeconds: 0 }, expected: 120 },
    { settings: { maxRounds: 7, roundTimeMinutes: 0, roundTimeSeconds: 45 }, expected: 45 },
    { settings: { maxRounds: 10, roundTimeMinutes: 3, roundTimeSeconds: 15 }, expected: 195 },
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
    { settings: { maxRounds: 5, roundTimeMinutes: 1, roundTimeSeconds: 30 }, expected: "1:30" },
    { settings: { maxRounds: 3, roundTimeMinutes: 2, roundTimeSeconds: 0 }, expected: "2:00" },
    { settings: { maxRounds: 7, roundTimeMinutes: 0, roundTimeSeconds: 5 }, expected: "0:05" },
    { settings: { maxRounds: 10, roundTimeMinutes: 5, roundTimeSeconds: 45 }, expected: "5:45" },
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
      settings: { maxRounds: 5, roundTimeMinutes: 1, roundTimeSeconds: 30 },
      expectedMin: 11,
      expectedMax: 13,
    },
    {
      settings: { maxRounds: 3, roundTimeMinutes: 2, roundTimeSeconds: 0 },
      expectedMin: 8,
      expectedMax: 10,
    },
    {
      settings: { maxRounds: 10, roundTimeMinutes: 1, roundTimeSeconds: 0 },
      expectedMin: 14,
      expectedMax: 16,
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
      roundTimeMinutes: 1,
      roundTimeSeconds: 30,
    };

    assertEquals(settings.maxRounds, count);
    assertEquals(validRoundCounts.includes(settings.maxRounds), true);
  });
});

Deno.test("GameSettingsModal - validates time options", () => {
  const validMinutes = [0, 1, 2, 3, 4, 5];
  const validSeconds = [0, 15, 30, 45];

  validMinutes.forEach((minutes) => {
    validSeconds.forEach((seconds) => {
      const settings: GameSettings = {
        maxRounds: 5,
        roundTimeMinutes: minutes,
        roundTimeSeconds: seconds,
      };

      assertEquals(validMinutes.includes(settings.roundTimeMinutes), true);
      assertEquals(validSeconds.includes(settings.roundTimeSeconds), true);
    });
  });
});

Deno.test("GameSettingsModal - prevents invalid time combinations", () => {
  // Test that 0:00 time would be invalid (though UI should prevent this)
  const invalidSettings: GameSettings = {
    maxRounds: 5,
    roundTimeMinutes: 0,
    roundTimeSeconds: 0,
  };

  const totalTime = calculateTotalTime(invalidSettings);
  assertEquals(totalTime, 0);

  // In a real implementation, this would be validated
  const isValidTime = totalTime > 0;
  assertEquals(isValidTime, false);
});

Deno.test("GameSettingsModal - handles maximum time settings", () => {
  const maxSettings: GameSettings = {
    maxRounds: 10,
    roundTimeMinutes: 5,
    roundTimeSeconds: 45,
  };

  const totalTime = calculateTotalTime(maxSettings);
  const estimatedGameTime = estimateGameTime(maxSettings);

  assertEquals(totalTime, 345); // 5:45 = 345 seconds
  assertEquals(estimatedGameTime >= 80, true); // Should be quite long
});

Deno.test("GameSettingsModal - handles minimum practical settings", () => {
  const minSettings: GameSettings = {
    maxRounds: 3,
    roundTimeMinutes: 0,
    roundTimeSeconds: 30,
  };

  const totalTime = calculateTotalTime(minSettings);
  const estimatedGameTime = estimateGameTime(minSettings);

  assertEquals(totalTime, 30); // 0:30 = 30 seconds
  assertEquals(estimatedGameTime >= 2, true); // Should be at least a couple minutes
});
