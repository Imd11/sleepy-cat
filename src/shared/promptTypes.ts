export type PromptItem = {
  id: string;
  title: string;
  body: string;
  order: number;
  createdAt: string;
  updatedAt: string;
};

export function normalizePromptTitle(title: string): string {
  return title.trim();
}

export function getPromptPreview(body: string, maxLength = 80): string {
  const compact = body.replace(/\s+/g, " ").trim();
  if (compact.length <= maxLength) return compact;
  return `${compact.slice(0, maxLength).trimEnd()}...`;
}