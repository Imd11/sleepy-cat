import { useState } from "react";
import type { PromptContainer } from "../shared/promptTypes";
import {
  getPromptContainerBodies,
  getPromptContainerMeta,
  getPromptContainerPreviewLines,
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
  const [hoveredPromptId, setHoveredPromptId] = useState<string | null>(null);
  const hoveredPrompt = prompts.find((prompt) => prompt.id === hoveredPromptId) ?? null;

  return (
    <div className="prompt-quick-shell">
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
              role="option"
              aria-selected="false"
              disabled={submittingPromptId === prompt.id}
              onMouseEnter={() => setHoveredPromptId(prompt.id)}
              onMouseLeave={() => setHoveredPromptId((current) => (
                current === prompt.id ? null : current
              ))}
              onFocus={() => setHoveredPromptId(prompt.id)}
              onBlur={() => setHoveredPromptId((current) => (
                current === prompt.id ? null : current
              ))}
              onClick={() => onSelect(prompt)}
            >
              <span className="prompt-quick-title-row">
                <span className="prompt-quick-title">{prompt.title}</span>
                <span className="prompt-quick-meta">{getPromptContainerMeta(prompt)}</span>
              </span>
              <span className="prompt-quick-preview-lines">
                {getPromptContainerPreviewLines(prompt).map((line) => (
                  <span className="prompt-quick-preview-line" key={line}>
                    {line}
                  </span>
                ))}
              </span>
            </button>
          ))
        )}
      </div>
      {hoveredPrompt ? <PromptHoverPreview prompt={hoveredPrompt} /> : null}
    </div>
  );
}

function PromptHoverPreview({ prompt }: { prompt: PromptContainer }) {
  const bodies = getPromptContainerBodies(prompt);

  return (
    <aside className="prompt-hover-preview" role="tooltip">
      <div className="prompt-hover-preview-header">
        <strong>{prompt.title}</strong>
        <span>{getPromptContainerMeta(prompt)}</span>
      </div>
      <div className="prompt-hover-preview-body">
        {prompt.type === "group" ? (
          bodies.map((body, index) => (
            <p key={`${index}-${body}`}>
              {index + 1}. {body}
            </p>
          ))
        ) : (
          <p>{bodies[0] ?? ""}</p>
        )}
      </div>
    </aside>
  );
}
