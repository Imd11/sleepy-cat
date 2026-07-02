import type { PromptContainer } from "../shared/promptTypes";
import {
  getPromptContainerMeta,
  getPromptContainerPreview,
} from "../shared/promptTypes";

interface PromptQuickListProps {
  prompts: PromptContainer[];
  onSelect: (prompt: PromptContainer) => void;
  submittingPromptId?: string | null;
}

export function PromptQuickList({
  prompts,
  onSelect,
  submittingPromptId = null,
}: PromptQuickListProps) {
  return (
    <div className="prompt-quick-list" role="listbox" aria-label="Prompts">
      {prompts.length === 0 ? (
        <div className="prompt-quick-empty">
          <strong>No prompts yet</strong>
          <span>Open Prompt Picker to create your first prompt.</span>
        </div>
      ) : (
        prompts.map((prompt) => (
          <button
            key={prompt.id}
            className={`prompt-quick-item ${
              prompt.type === "group" ? "prompt-quick-item-group" : ""
            }`}
            type="button"
            disabled={submittingPromptId === prompt.id}
            onClick={() => onSelect(prompt)}
          >
            <span className="prompt-quick-title-row">
              <span className="prompt-quick-title">{prompt.title}</span>
              <span className="prompt-quick-meta">{getPromptContainerMeta(prompt)}</span>
            </span>
            <span className="prompt-quick-preview">
              {getPromptContainerPreview(prompt)}
            </span>
          </button>
        ))
      )}
    </div>
  );
}
