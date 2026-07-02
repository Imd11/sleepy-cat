import { useState } from "react";
import type { PromptContainer } from "../shared/promptTypes";
import {
  DEFAULT_GROUP_INTERVAL_MS,
  getPromptContainerMeta,
  getPromptContainerPreview,
} from "../shared/promptTypes";

type EditorMode = "single" | "group";

type Draft = {
  title: string;
  type: EditorMode;
  body: string;
  prompts: string[];
  intervalMs: number;
};

interface PromptManagerProps {
  prompts: PromptContainer[];
  onCreate: (input: { title: string; body: string }) => void;
  onCreateGroup: (input: {
    title: string;
    prompts: Array<{ body: string }>;
    intervalMs: number;
  }) => void;
  onUpdate: (
    id: string,
    input: {
      title?: string;
      body?: string;
      type?: EditorMode;
      prompts?: Array<{ body: string; order: number }>;
      intervalMs?: number;
    }
  ) => void;
  onDelete: (id: string) => void;
  onReorder: (orderedIds: string[]) => void;
  onImport: () => void;
  onExport: () => void;
}

const emptyDraft = (): Draft => ({
  title: "",
  type: "single",
  body: "",
  prompts: ["", ""],
  intervalMs: DEFAULT_GROUP_INTERVAL_MS,
});

function draftFromPrompt(prompt: PromptContainer): Draft {
  const orderedPrompts = [...prompt.prompts].sort((a, b) => a.order - b.order);
  return {
    title: prompt.title,
    type: prompt.type,
    body: orderedPrompts[0]?.body ?? "",
    prompts: prompt.type === "group"
      ? orderedPrompts.map((entry) => entry.body)
      : [orderedPrompts[0]?.body ?? "", ""],
    intervalMs: prompt.intervalMs,
  };
}

function cleanBodies(bodies: string[]): Array<{ body: string; order: number }> {
  return bodies
    .map((body) => body.trim())
    .filter(Boolean)
    .map((body, order) => ({ body, order }));
}

function hasValidDraft(draft: Draft): boolean {
  if (!draft.title.trim()) return false;
  if (draft.type === "single") return Boolean(draft.body.trim());
  return cleanBodies(draft.prompts).length > 0;
}

export function PromptManager({
  prompts,
  onCreate,
  onCreateGroup,
  onUpdate,
  onDelete,
  onReorder,
  onImport,
  onExport
}: PromptManagerProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft>(() => emptyDraft());
  const [editDraft, setEditDraft] = useState<Draft>(() => emptyDraft());

  const setDraftPrompt = (index: number, value: string) => {
    const next = [...draft.prompts];
    next[index] = value;
    setDraft({ ...draft, prompts: next });
  };

  const setEditPrompt = (index: number, value: string) => {
    const next = [...editDraft.prompts];
    next[index] = value;
    setEditDraft({ ...editDraft, prompts: next });
  };

  const handleCreate = () => {
    if (!hasValidDraft(draft)) return;
    if (draft.type === "group") {
      onCreateGroup({
        title: draft.title.trim(),
        prompts: cleanBodies(draft.prompts),
        intervalMs: draft.intervalMs,
      });
    } else {
      onCreate({ title: draft.title.trim(), body: draft.body.trim() });
    }
    setDraft(emptyDraft());
  };

  const handleSaveEdit = (id: string) => {
    if (!hasValidDraft(editDraft)) return;
    if (editDraft.type === "group") {
      onUpdate(id, {
        title: editDraft.title.trim(),
        type: "group",
        prompts: cleanBodies(editDraft.prompts),
        intervalMs: editDraft.intervalMs,
      });
    } else {
      onUpdate(id, {
        title: editDraft.title.trim(),
        type: "single",
        body: editDraft.body.trim(),
      });
    }
    setEditingId(null);
    setEditDraft(emptyDraft());
  };

  const handleMoveUp = (index: number) => {
    if (index === 0) return;
    const newOrder = [...prompts.map((p) => p.id)];
    [newOrder[index - 1], newOrder[index]] = [newOrder[index], newOrder[index - 1]];
    onReorder(newOrder);
  };

  const handleMoveDown = (index: number) => {
    if (index === prompts.length - 1) return;
    const newOrder = [...prompts.map((p) => p.id)];
    [newOrder[index], newOrder[index + 1]] = [newOrder[index + 1], newOrder[index]];
    onReorder(newOrder);
  };

  return (
    <div className="prompt-manager page-stack">
      <header className="page-header">
        <div>
          <h1>Manage Prompts</h1>
          <p>{prompts.length} prompt containers in your local library.</p>
        </div>
        <div className="toolbar">
          <button className="button button-secondary" onClick={onImport}>
            Import
          </button>
          <button className="button button-secondary" onClick={onExport}>
            Export
          </button>
        </div>
      </header>

      <section className="editor-panel editor-panel-stacked">
        <div className="section-heading">
          <h2>New Prompt Container</h2>
          <p>Add one prompt or an ordered group for the quick picker.</p>
        </div>
        <div className="segmented-control" aria-label="Prompt container type">
          <button
            className={draft.type === "single" ? "is-selected" : ""}
            type="button"
            onClick={() => setDraft({ ...draft, type: "single" })}
          >
            Single
          </button>
          <button
            className={draft.type === "group" ? "is-selected" : ""}
            type="button"
            onClick={() => setDraft({ ...draft, type: "group" })}
          >
            Group
          </button>
        </div>
        <input
          className="field"
          type="text"
          placeholder="Title"
          value={draft.title}
          onChange={(e) => setDraft({ ...draft, title: e.target.value })}
        />
        {draft.type === "single" ? (
          <textarea
            className="field prompt-body-field"
            placeholder="Prompt body..."
            value={draft.body}
            onChange={(e) => setDraft({ ...draft, body: e.target.value })}
          />
        ) : (
          <GroupFields
            prompts={draft.prompts}
            intervalMs={draft.intervalMs}
            onIntervalChange={(intervalMs) => setDraft({ ...draft, intervalMs })}
            onPromptChange={setDraftPrompt}
            onAddPrompt={() => setDraft({ ...draft, prompts: [...draft.prompts, ""] })}
            onRemovePrompt={(index) => {
              const next = draft.prompts.filter((_, i) => i !== index);
              setDraft({ ...draft, prompts: next.length ? next : [""] });
            }}
            onMovePrompt={(from, to) => {
              const next = [...draft.prompts];
              [next[from], next[to]] = [next[to], next[from]];
              setDraft({ ...draft, prompts: next });
            }}
          />
        )}
        <button className="button button-primary editor-submit" onClick={handleCreate}>
          Add {draft.type === "group" ? "Group" : "Prompt"}
        </button>
      </section>

      <section className="list-panel">
        <div className="section-heading">
          <h2>Prompt List</h2>
          <p>Choose the order used by the floating picker.</p>
        </div>
        <div className="prompt-list">
          {prompts.length === 0 ? (
            <div className="empty-state-block">No prompts yet</div>
          ) : prompts.map((prompt, index) => (
            <div
              key={prompt.id}
              className={`prompt-item ${prompt.type === "group" ? "prompt-item-group" : ""}`}
            >
              {editingId === prompt.id ? (
                <div className="edit-form edit-form-stacked">
                  <div className="segmented-control" aria-label="Edit prompt container type">
                    <button
                      className={editDraft.type === "single" ? "is-selected" : ""}
                      type="button"
                      onClick={() => setEditDraft({ ...editDraft, type: "single" })}
                    >
                      Single
                    </button>
                    <button
                      className={editDraft.type === "group" ? "is-selected" : ""}
                      type="button"
                      onClick={() => setEditDraft({ ...editDraft, type: "group" })}
                    >
                      Group
                    </button>
                  </div>
                  <input
                    className="field"
                    type="text"
                    value={editDraft.title}
                    onChange={(e) => setEditDraft({ ...editDraft, title: e.target.value })}
                  />
                  {editDraft.type === "single" ? (
                    <textarea
                      className="field prompt-body-field"
                      value={editDraft.body}
                      onChange={(e) => setEditDraft({ ...editDraft, body: e.target.value })}
                    />
                  ) : (
                    <GroupFields
                      prompts={editDraft.prompts}
                      intervalMs={editDraft.intervalMs}
                      onIntervalChange={(intervalMs) => setEditDraft({ ...editDraft, intervalMs })}
                      onPromptChange={setEditPrompt}
                      onAddPrompt={() =>
                        setEditDraft({ ...editDraft, prompts: [...editDraft.prompts, ""] })
                      }
                      onRemovePrompt={(entryIndex) => {
                        const next = editDraft.prompts.filter((_, i) => i !== entryIndex);
                        setEditDraft({ ...editDraft, prompts: next.length ? next : [""] });
                      }}
                      onMovePrompt={(from, to) => {
                        const next = [...editDraft.prompts];
                        [next[from], next[to]] = [next[to], next[from]];
                        setEditDraft({ ...editDraft, prompts: next });
                      }}
                    />
                  )}
                  <div className="edit-actions">
                    <button
                      className="button button-primary"
                      onClick={() => handleSaveEdit(prompt.id)}
                    >
                      Save
                    </button>
                    <button
                      className="button button-secondary"
                      onClick={() => setEditingId(null)}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              ) : deleteConfirmId === prompt.id ? (
                <div className="delete-confirm">
                  <span>Delete this prompt?</span>
                  <div className="confirm-actions">
                    <button
                      className="button button-danger"
                      onClick={() => { onDelete(prompt.id); setDeleteConfirmId(null); }}
                    >
                      Confirm
                    </button>
                    <button
                      className="button button-secondary"
                      onClick={() => setDeleteConfirmId(null)}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              ) : (
                <div className="prompt-content">
                  <div className="prompt-info">
                    <div className="prompt-title-row">
                      <strong>{prompt.title}</strong>
                      <span className="prompt-kind-badge">{getPromptContainerMeta(prompt)}</span>
                    </div>
                    <span className="prompt-preview">
                      {getPromptContainerPreview(prompt, 140)}
                    </span>
                  </div>
                  <div className="prompt-actions">
                    <button
                      className="button icon-button"
                      onClick={() => handleMoveUp(index)}
                      disabled={index === 0}
                    >
                      ↑
                    </button>
                    <button
                      className="button icon-button"
                      onClick={() => handleMoveDown(index)}
                      disabled={index === prompts.length - 1}
                    >
                      ↓
                    </button>
                    <button
                      className="button button-secondary"
                      onClick={() => {
                        setEditingId(prompt.id);
                        setEditDraft(draftFromPrompt(prompt));
                      }}
                    >
                      Edit
                    </button>
                    <button
                      className="button button-ghost-danger"
                      onClick={() => setDeleteConfirmId(prompt.id)}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

interface GroupFieldsProps {
  prompts: string[];
  intervalMs: number;
  onIntervalChange: (intervalMs: number) => void;
  onPromptChange: (index: number, value: string) => void;
  onAddPrompt: () => void;
  onRemovePrompt: (index: number) => void;
  onMovePrompt: (from: number, to: number) => void;
}

function GroupFields({
  prompts,
  intervalMs,
  onIntervalChange,
  onPromptChange,
  onAddPrompt,
  onRemovePrompt,
  onMovePrompt,
}: GroupFieldsProps) {
  return (
    <div className="group-editor">
      <label className="interval-field">
        <span>Delay between prompts</span>
        <input
          className="field"
          type="number"
          min={200}
          max={3000}
          step={50}
          value={intervalMs}
          onChange={(e) => onIntervalChange(Number(e.target.value))}
        />
        <span>ms</span>
      </label>
      <div className="group-prompt-list">
        {prompts.map((body, index) => (
          <div className="group-prompt-row" key={index}>
            <label>Prompt {index + 1}</label>
            <textarea
              className="field prompt-body-field"
              value={body}
              onChange={(e) => onPromptChange(index, e.target.value)}
            />
            <div className="group-prompt-actions">
              <button
                className="button icon-button"
                type="button"
                disabled={index === 0}
                onClick={() => onMovePrompt(index, index - 1)}
              >
                ↑
              </button>
              <button
                className="button icon-button"
                type="button"
                disabled={index === prompts.length - 1}
                onClick={() => onMovePrompt(index, index + 1)}
              >
                ↓
              </button>
              <button
                className="button button-secondary"
                type="button"
                onClick={() => onRemovePrompt(index)}
              >
                Remove
              </button>
            </div>
          </div>
        ))}
      </div>
      <button className="button button-secondary" type="button" onClick={onAddPrompt}>
        Add Prompt
      </button>
    </div>
  );
}
