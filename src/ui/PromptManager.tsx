import { useState } from "react";
import type { PromptItem } from "../shared/promptTypes";

interface PromptManagerProps {
  prompts: PromptItem[];
  onCreate: (input: { title: string; body: string }) => void;
  onUpdate: (id: string, input: { title?: string; body?: string }) => void;
  onDelete: (id: string) => void;
  onReorder: (orderedIds: string[]) => void;
  onImport: () => void;
  onExport: () => void;
}

export function PromptManager({
  prompts,
  onCreate,
  onUpdate,
  onDelete,
  onReorder,
  onImport,
  onExport
}: PromptManagerProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [newTitle, setNewTitle] = useState("");
  const [newBody, setNewBody] = useState("");

  const handleCreate = () => {
    if (newTitle.trim() && newBody.trim()) {
      onCreate({ title: newTitle.trim(), body: newBody.trim() });
      setNewTitle("");
      setNewBody("");
    }
  };

  const handleSaveEdit = (id: string) => {
    if (newTitle.trim() && newBody.trim()) {
      onUpdate(id, { title: newTitle.trim(), body: newBody.trim() });
      setEditingId(null);
      setNewTitle("");
      setNewBody("");
    }
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
    <div className="prompt-manager">
      <h2>Manage Prompts</h2>

      <div className="prompt-editor">
        <input
          type="text"
          placeholder="Title"
          value={newTitle}
          onChange={(e) => setNewTitle(e.target.value)}
        />
        <textarea
          placeholder="Prompt body..."
          value={newBody}
          onChange={(e) => setNewBody(e.target.value)}
        />
        <button onClick={handleCreate}>Add Prompt</button>
      </div>

      <div className="prompt-list">
        {prompts.map((prompt, index) => (
          <div key={prompt.id} className="prompt-item">
            {editingId === prompt.id ? (
              <div className="edit-form">
                <input
                  type="text"
                  value={newTitle}
                  onChange={(e) => setNewTitle(e.target.value)}
                />
                <textarea
                  value={newBody}
                  onChange={(e) => setNewBody(e.target.value)}
                />
                <div className="edit-actions">
                  <button onClick={() => handleSaveEdit(prompt.id)}>Save</button>
                  <button onClick={() => setEditingId(null)}>Cancel</button>
                </div>
              </div>
            ) : deleteConfirmId === prompt.id ? (
              <div className="delete-confirm">
                <span>Delete this prompt?</span>
                <div className="confirm-actions">
                  <button onClick={() => { onDelete(prompt.id); setDeleteConfirmId(null); }}>
                    Confirm
                  </button>
                  <button onClick={() => setDeleteConfirmId(null)}>Cancel</button>
                </div>
              </div>
            ) : (
              <div className="prompt-content">
                <div className="prompt-info">
                  <strong>{prompt.title}</strong>
                  <span className="prompt-preview">{prompt.body.slice(0, 50)}...</span>
                </div>
                <div className="prompt-actions">
                  <button onClick={() => handleMoveUp(index)} disabled={index === 0}>↑</button>
                  <button onClick={() => handleMoveDown(index)} disabled={index === prompts.length - 1}>↓</button>
                  <button onClick={() => { setEditingId(prompt.id); setNewTitle(prompt.title); setNewBody(prompt.body); }}>
                    Edit
                  </button>
                  <button onClick={() => setDeleteConfirmId(prompt.id)}>Delete</button>
                </div>
              </div>
            )}
          </div>
        ))}
      </div>

      <div className="manager-footer">
        <button onClick={onImport}>Import</button>
        <button onClick={onExport}>Export</button>
      </div>
    </div>
  );
}