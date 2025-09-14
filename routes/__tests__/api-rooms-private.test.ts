import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { handler as roomsHandler } from "../api/rooms.ts";
import { getKVRoomService } from "../../lib/db/index.ts";

async function withApiTest(
  name: string,
  testFn: () => Promise<void>,
) {
  const mockEnv = {
    ENVIRONMENT: "test",
  };

  Deno.test(name, async () => {
    Object.entries(mockEnv).forEach(([key, value]) => Deno.env.set(key, value));
    const kvRoomService = getKVRoomService();
    await kvRoomService.deleteAllRooms();

    try {
      await testFn();
    } finally {
      await kvRoomService.deleteAllRooms();
      kvRoomService.close();
      Object.keys(mockEnv).forEach((key) => Deno.env.delete(key));
    }
  });
}


/**
 * API endpoint tests for private room functionality
 */

withApiTest("API Rooms - POST creates private room", async () => {
  const requestBody = {
    name: "Test Private Room",
    hostName: "Test Host",
    gameType: "drawing",
    maxPlayers: 6,
    isPrivate: true,
  };

  const request = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);
  assertEquals(response.status, 201);

  const data = await response.json();
  assertExists(data.roomId);
  assertExists(data.playerId);
  assertExists(data.room);

  // Verify room properties
  assertEquals(data.room.room.name, requestBody.name);
  assertEquals(data.room.room.isPrivate, true);
  assertEquals(data.room.room.gameType, requestBody.gameType);
  assertEquals(data.room.room.maxPlayers, requestBody.maxPlayers);
});

withApiTest("API Rooms - POST creates public room by default", async () => {
  const requestBody = {
    name: "Test Public Room",
    hostName: "Public Host",
    gameType: "drawing",
    maxPlayers: 8,
  };

  const request = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);
  assertEquals(response.status, 201);

  const data = await response.json();
  assertExists(data.room);

  // Verify room is public by default
  assertEquals(data.room.room.isPrivate, false);
});

withApiTest("API Rooms - POST creates public room when isPrivate is false", async () => {
  const requestBody = {
    name: "Test Explicit Public Room",
    hostName: "Explicit Public Host",
    gameType: "drawing",
    maxPlayers: 8,
    isPrivate: false,
  };

  const request = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);
  assertEquals(response.status, 201);

  const data = await response.json();
  assertExists(data.room);

  // Verify room is explicitly public
  assertEquals(data.room.room.isPrivate, false);
});

withApiTest("API Rooms - GET filters out private rooms", async () => {
  // First, create a public room
  const publicRequestBody = {
    name: "Public API Test Room",
    hostName: "Public API Host",
    gameType: "drawing",
    maxPlayers: 8,
    isPrivate: false,
  };

  const publicRequest = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(publicRequestBody),
  });

  const publicResponse = await roomsHandler.POST!(publicRequest, {} as any);
  assertEquals(publicResponse.status, 201);

  // Create a private room
  const privateRequestBody = {
    name: "Private API Test Room",
    hostName: "Private API Host",
    gameType: "drawing",
    maxPlayers: 8,
    isPrivate: true,
  };

  const privateRequest = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(privateRequestBody),
  });

  const privateResponse = await roomsHandler.POST!(privateRequest, {} as any);
  assertEquals(privateResponse.status, 201);

  // Now GET rooms - should only return public room
  const getRequest = new Request("http://localhost:3000/api/rooms");
  const getResponse = await roomsHandler.GET!(getRequest, {} as any);
  assertEquals(getResponse.status, 200);

  const getData = await getResponse.json();
  assertExists(getData.rooms);

  // Should only contain the public room
  const rooms = getData.rooms;
  assertEquals(rooms.length, 1);
  assertEquals(rooms[0].room.name, "Public API Test Room");
  assertEquals(rooms[0].room.isPrivate, false);
});

withApiTest("API Rooms - POST validation with missing required fields", async () => {
  // Test missing room name
  const requestBody = {
    hostName: "Test Host",
    gameType: "drawing",
    maxPlayers: 8,
    isPrivate: true,
  };

  const request = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);
  assertEquals(response.status, 400);

  const data = await response.json();
  assertExists(data.error);
  assertEquals(data.error, "Room name and host name are required");
});

withApiTest("API Rooms - POST validation with missing host name", async () => {
  // Test missing host name
  const requestBody = {
    name: "Test Room",
    gameType: "drawing",
    maxPlayers: 8,
    isPrivate: true,
  };

  const request = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);
  assertEquals(response.status, 400);

  const data = await response.json();
  assertExists(data.error);
  assertEquals(data.error, "Room name and host name are required");
});

withApiTest("API Rooms - POST defaults to public when isPrivate not provided", async () => {
  const requestBody = {
    name: "Default Public Room",
    hostName: "Default Host",
    gameType: "drawing",
    maxPlayers: 8,
  };

  const request = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);
  assertEquals(response.status, 201);

  const data = await response.json();
  assertExists(data.room);

  // Verify room defaults to public
  assertEquals(data.room.room.isPrivate, false);
});

withApiTest("API Rooms - POST handles isPrivate as string 'false'", async () => {
  // Test when isPrivate comes as string (common in form submissions)
  const requestBody = {
    name: "String False Room",
    hostName: "String Host",
    gameType: "drawing",
    maxPlayers: 8,
    isPrivate: "false", // String instead of boolean
  };

  const request = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);
  assertEquals(response.status, 201);

  const data = await response.json();
  assertExists(data.room);

  // Should be treated as false (public room)
  assertEquals(data.room.room.isPrivate, false);
});
