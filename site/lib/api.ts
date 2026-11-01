import { SERVER_URL } from "./env";

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

// Errors from the license server are `{ error, message }`; the message is written for people.
export async function api<T>(path: string, body?: unknown): Promise<T> {
  let res: Response;
  try {
    res = await fetch(SERVER_URL + path, {
      method: body === undefined ? "GET" : "POST",
      headers: body === undefined ? undefined : { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      cache: "no-store",
    });
  } catch {
    throw new ApiError("Can't reach the license server. Check your connection and try again.", 0);
  }
  const json = await res.json().catch(() => null);
  if (!res.ok) {
    throw new ApiError(json?.message ?? "Something went wrong. Try again in a moment.", res.status);
  }
  return json as T;
}
