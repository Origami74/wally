import type { SessionStatusType } from "./types";

export const formatBytes = (bytes?: number | null) => {
  if (!bytes || bytes <= 0) return "--";
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), sizes.length - 1);
  const value = bytes / Math.pow(1024, i);
  return `${value.toFixed(1)} ${sizes[i]}`;
};

export const formatDuration = (seconds?: number | null) => {
  if (!seconds || seconds <= 0) return "--";
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
};

export const statusTone = (
  status: SessionStatusType | string,
): "success" | "warning" | "danger" | "info" | "default" => {
  switch (status.toString().toLowerCase()) {
    case "active":
      return "success";
    case "renewing":
      return "warning";
    case "expired":
    case "error":
      return "danger";
    case "available":
      return "info";
    default:
      return "default";
  }
};
