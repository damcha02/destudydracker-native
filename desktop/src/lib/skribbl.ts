import type { SocialState } from "../types";

const DEFAULT_SOCIAL_API_URL = "https://study-tracker-social.danil-poluyanov13.workers.dev";
const SOCIAL_API_URL = (import.meta.env.VITE_SOCIAL_API_URL || DEFAULT_SOCIAL_API_URL).replace(/\/$/, "");

export const SKRIBBL_DRAW_SECONDS = 3 * 60;
export const SKRIBBL_GALLERY_PAGE_SIZE = 16;

export interface SkribblTheme {
  date: string;
  theme: string;
  submitted: boolean;
  drawingId: string | null;
  imageUrl: string | null;
}

export interface SkribblDrawing {
  id: string;
  userId: string;
  displayName: string;
  voteScore: number;
  voteCount: number;
  myVote: number | null;
  isSelf: boolean;
  imageUrl: string;
  createdAt: string;
}

export interface SkribblGalleryPage {
  date: string;
  drawings: SkribblDrawing[];
  total: number;
  nextOffset: number | null;
  hasMore: boolean;
}

export interface SkribblWinner {
  date: string;
  drawingId: string;
  userId: string;
  displayName: string;
  score: number;
  imageUrl: string | null;
}

export interface SkribblLeaderboard {
  date: string;
  winner: SkribblWinner | null;
}

async function requestSkribbl<T>(path: string, init: RequestInit): Promise<T> {
  if (!SOCIAL_API_URL) throw new Error("Social sync is not configured. Add VITE_SOCIAL_API_URL after deploying the Cloudflare Worker.");
  const response = await fetch(`${SOCIAL_API_URL}${path}`, init);
  if (!response.ok) {
    const detail = await response.text().catch(() => "");
    throw new Error(detail || `Skribbl request failed with HTTP ${response.status}.`);
  }
  return response.json() as Promise<T>;
}

function identityParams(social: SocialState) {
  const params = new URLSearchParams({ userId: social.userId, deviceSecret: social.deviceSecret });
  return params.toString();
}

function jsonBody(body: Record<string, unknown>) {
  return {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  };
}

export function getSkribblTheme(social: SocialState) {
  return requestSkribbl<SkribblTheme>(`/skribbl/theme?${identityParams(social)}`, { method: "GET" });
}

export function getSkribblGallery(social: SocialState, date: string, offset = 0) {
  return requestSkribbl<SkribblGalleryPage>(
    "/skribbl/gallery",
    jsonBody({
      userId: social.userId,
      deviceSecret: social.deviceSecret,
      date,
      offset,
      limit: SKRIBBL_GALLERY_PAGE_SIZE,
    }),
  );
}

export function getSkribblLeaderboard(social: SocialState) {
  return requestSkribbl<SkribblLeaderboard>(`/skribbl/leaderboard?${identityParams(social)}`, { method: "GET" });
}

export function submitSkribblDrawing(social: SocialState, image: Blob, date: string) {
  const form = new FormData();
  form.set("userId", social.userId);
  form.set("deviceSecret", social.deviceSecret);
  form.set("date", date);
  form.set("image", image, "drawing.webp");
  return requestSkribbl<{ ok: boolean; drawingId: string; date: string; imageUrl: string }>("/skribbl/submit", {
    method: "POST",
    body: form,
  });
}

export function voteSkribblDrawing(social: SocialState, drawingId: string, vote: -1 | 0 | 1) {
  return requestSkribbl<{ ok: boolean; score: number }>(
    "/skribbl/vote",
    jsonBody({ userId: social.userId, deviceSecret: social.deviceSecret, drawingId, vote }),
  );
}
