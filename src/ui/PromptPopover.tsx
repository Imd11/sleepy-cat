import type { PromptItem } from "../shared/promptTypes";
import { getPromptPreview } from "../shared/promptTypes";

interface PromptPopoverProps {
  prompts: PromptItem[];
  onSelect: (prompt: PromptItem) => void;
  onManage: () => void;
}

export function PromptPopover({ prompts, onSelect, onManage }: PromptPopoverProps) {
  return (
    <div className="prompt-popover">
      <div className="prompt-list">
        {prompts.map((prompt) => (
          <div
            key={prompt.id}
            className="prompt-item"
            onClick={() => onSelect(prompt)}
          >
            <div className="prompt-title">{prompt.title}</div>
            <div className="prompt-preview">{getPromptPreview(prompt.body)}</div>
          </div>
        ))}
        {prompts.length === 0 && (
          <div className="prompt-empty">No prompts yet</div>
        )}
      </div>
      <div className="prompt-footer">
        <button className="manage-btn" onClick={onManage}>
          Manage Prompts
        </button>
      </div>
    </div>
  );
}