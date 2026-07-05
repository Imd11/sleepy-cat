import { useEffect, useRef, useState, type DragEvent } from "react";
import type { PromptCategory, PromptContainer } from "../shared/promptTypes";
import {
  DEFAULT_GROUP_INTERVAL_MS,
  MAX_GROUP_INTERVAL_MS,
  MIN_GROUP_INTERVAL_MS,
  getPromptContainerBodies,
  getPromptContainerPreviewLines,
} from "../shared/promptTypes";
import type { Messages } from "../shared/i18n";
import { CategoryRail } from "./CategoryRail";

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
  categories: PromptCategory[];
  activeCategoryId: string | null;
  categoryCounts: Record<string, number>;
  totalPromptCount: number;
  messages: Messages;
  onOpenSettings: () => void;
  onSelectCategory: (categoryId: string) => void;
  onCreateCategory: (name: string) => void | Promise<void>;
  onRenameCategory: (categoryId: string, name: string) => void | Promise<void>;
  onDeleteCategory: (categoryId: string) => void | Promise<void>;
  getCategoryDisplayName: (category: PromptCategory) => string;
  categoryActionError?: string | null;
  onCreate: (input: { title: string; body: string }) => void | Promise<void>;
  onCreateGroup: (input: {
    title: string;
    prompts: Array<{ body: string }>;
    intervalMs: number;
  }) => void | Promise<void>;
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

const GROUP_INTERVAL_STEP_SECONDS = 0.1;

function formatIntervalSeconds(intervalMs: number): string {
  return String(Number((intervalMs / 1000).toFixed(2)));
}

function intervalSecondsToMs(seconds: number): number {
  if (!Number.isFinite(seconds)) return MIN_GROUP_INTERVAL_MS;
  const minSeconds = MIN_GROUP_INTERVAL_MS / 1000;
  const maxSeconds = MAX_GROUP_INTERVAL_MS / 1000;
  const clampedSeconds = Math.min(maxSeconds, Math.max(minSeconds, seconds));
  return Math.round(clampedSeconds * 1000);
}

function moveArrayItem<T>(items: T[], from: number, to: number): T[] {
  if (from === to || from < 0 || to < 0 || from >= items.length || to >= items.length) {
    return items;
  }
  const next = [...items];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

function PromptKindBadge({ prompt, messages }: { prompt: PromptContainer; messages: Messages }) {
  if (prompt.type !== "group") return null;
  const count = getPromptContainerBodies(prompt).length;
  return (
    <span className="prompt-kind-badge">
      {messages.manager.groupMeta(count)}
    </span>
  );
}

export function PromptManager({
  prompts,
  categories,
  activeCategoryId,
  categoryCounts,
  totalPromptCount,
  messages,
  onOpenSettings,
  onSelectCategory,
  onCreateCategory,
  onRenameCategory,
  onDeleteCategory,
  getCategoryDisplayName,
  categoryActionError = null,
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
  const [createPanelOpen, setCreatePanelOpen] = useState(false);
  const [createToastMessage, setCreateToastMessage] = useState<string | null>(null);
  const titleInputRef = useRef<HTMLInputElement | null>(null);
  const bodyTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const groupPromptRefs = useRef<Array<HTMLTextAreaElement | null>>([]);
  const editTitleInputRef = useRef<HTMLInputElement | null>(null);
  const editBodyTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const editGroupPromptRefs = useRef<Array<HTMLTextAreaElement | null>>([]);
  const submitGuardRef = useRef(false);
  const submitGuardTimerRef = useRef<number | null>(null);
  const createToastTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (submitGuardTimerRef.current !== null) {
        window.clearTimeout(submitGuardTimerRef.current);
      }
      if (createToastTimerRef.current !== null) {
        window.clearTimeout(createToastTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    setEditingId(null);
    setDeleteConfirmId(null);
    setCreatePanelOpen(false);
    setDraft(emptyDraft());
  }, [activeCategoryId]);

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

  const draftFromCreateDom = (): Draft => ({
    ...draft,
    title: titleInputRef.current?.value ?? draft.title,
    body: bodyTextareaRef.current?.value ?? draft.body,
    prompts: draft.prompts.map((value, index) => groupPromptRefs.current[index]?.value ?? value),
  });

  const draftFromEditDom = (): Draft => ({
    ...editDraft,
    title: editTitleInputRef.current?.value ?? editDraft.title,
    body: editBodyTextareaRef.current?.value ?? editDraft.body,
    prompts: editDraft.prompts.map((value, index) =>
      editGroupPromptRefs.current[index]?.value ?? value
    ),
  });

  const runSubmitOnce = (callback: () => void | Promise<void>) => {
    if (submitGuardRef.current) return;
    submitGuardRef.current = true;
    if (submitGuardTimerRef.current !== null) {
      window.clearTimeout(submitGuardTimerRef.current);
    }
    submitGuardTimerRef.current = window.setTimeout(() => {
      submitGuardRef.current = false;
      submitGuardTimerRef.current = null;
    }, 250);
    Promise.resolve(callback()).catch((error) => {
      console.error("Prompt manager action failed:", error);
    });
  };

  const showCreateToast = (message: string) => {
    setCreateToastMessage(message);
    if (createToastTimerRef.current !== null) {
      window.clearTimeout(createToastTimerRef.current);
    }
    createToastTimerRef.current = window.setTimeout(() => {
      setCreateToastMessage(null);
      createToastTimerRef.current = null;
    }, 2200);
  };

  const handleCreate = async (sourceDraft = draft) => {
    if (!hasValidDraft(sourceDraft)) return;
    if (sourceDraft.type === "group") {
      await onCreateGroup({
        title: sourceDraft.title.trim(),
        prompts: cleanBodies(sourceDraft.prompts),
        intervalMs: sourceDraft.intervalMs,
      });
      showCreateToast(messages.manager.groupAdded);
    } else {
      await onCreate({ title: sourceDraft.title.trim(), body: sourceDraft.body.trim() });
      showCreateToast(messages.manager.promptAdded);
    }
    setDraft(emptyDraft());
    setCreatePanelOpen(false);
  };

  const openCreatePanel = () => {
    setCreatePanelOpen(true);
    window.requestAnimationFrame(() => {
      titleInputRef.current?.focus();
      titleInputRef.current?.select();
    });
  };

  const cancelCreate = () => {
    setDraft(emptyDraft());
    setCreatePanelOpen(false);
  };

  const handleSaveEdit = (id: string, sourceDraft = editDraft) => {
    if (!hasValidDraft(sourceDraft)) return;
    if (sourceDraft.type === "group") {
      onUpdate(id, {
        title: sourceDraft.title.trim(),
        type: "group",
        prompts: cleanBodies(sourceDraft.prompts),
        intervalMs: sourceDraft.intervalMs,
      });
    } else {
      onUpdate(id, {
        title: sourceDraft.title.trim(),
        type: "single",
        body: sourceDraft.body.trim(),
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
          <h1>{messages.manager.title}</h1>
          <p>{messages.manager.count(totalPromptCount)}</p>
        </div>
        <div className="toolbar">
          <button className="button button-secondary" onClick={onOpenSettings}>
            {messages.common.settings}
          </button>
          <button className="button button-secondary" onClick={onImport}>
            {messages.common.import}
          </button>
          <button className="button button-secondary" onClick={onExport}>
            {messages.common.export}
          </button>
        </div>
      </header>

      <div className="prompt-manager-body">
        <CategoryRail
          categories={categories}
          activeCategoryId={activeCategoryId}
          counts={categoryCounts}
          messages={{
            title: messages.manager.categoriesTitle,
            newCategory: messages.manager.newCategory,
            newCategoryName: messages.manager.newCategoryName,
            newCategoryDefaultName: messages.manager.newCategoryDefaultName,
            categoryActions: messages.manager.categoryActions,
            renameCategory: messages.manager.renameCategory,
            deleteCategory: messages.manager.deleteCategory,
            saveCategory: messages.manager.saveCategory,
            cancelCategory: messages.manager.cancelCategory,
          }}
          getCategoryDisplayName={getCategoryDisplayName}
          actionError={categoryActionError}
          onSelect={onSelectCategory}
          onCreate={onCreateCategory}
          onRename={onRenameCategory}
          onDelete={onDeleteCategory}
        />
        <div className="prompt-manager-content">
          <section className="list-panel">
            <div className="section-heading panel-heading-with-actions">
              <div>
                <h2>{messages.manager.promptListTitle}</h2>
              </div>
              <button
                className="button button-primary list-add-button"
                type="button"
                onClick={openCreatePanel}
              >
                + {messages.manager.addPrompt}
              </button>
            </div>
            {createPanelOpen ? (
              <form
                className="editor-panel editor-panel-stacked create-panel-inline"
                onSubmit={(event) => {
                  event.preventDefault();
                  runSubmitOnce(() => handleCreate(draftFromCreateDom()));
                }}
              >
                <div className="section-heading panel-heading-with-actions">
                  <div>
                    <h2>{messages.manager.newContainerTitle}</h2>
                  </div>
                  <div
                    className="segmented-control"
                    aria-label={messages.manager.promptContainerType}
                  >
                    <button
                      className={draft.type === "single" ? "is-selected" : ""}
                      type="button"
                      aria-pressed={draft.type === "single"}
                      onClick={() => setDraft({ ...draft, type: "single" })}
                    >
                      {messages.manager.single}
                    </button>
                    <button
                      className={draft.type === "group" ? "is-selected" : ""}
                      type="button"
                      aria-pressed={draft.type === "group"}
                      onClick={() => setDraft({ ...draft, type: "group" })}
                    >
                      {messages.manager.group}
                    </button>
                  </div>
                </div>
                <input
                  ref={titleInputRef}
                  className="field"
                  type="text"
                  placeholder={messages.manager.titlePlaceholder}
                  value={draft.title}
                  onChange={(e) => setDraft({ ...draft, title: e.target.value })}
                />
                {draft.type === "single" ? (
                  <textarea
                    ref={bodyTextareaRef}
                    className="field prompt-body-field"
                    placeholder={messages.manager.bodyPlaceholder}
                    value={draft.body}
                    onChange={(e) => setDraft({ ...draft, body: e.target.value })}
                  />
                ) : (
                  <GroupFields
                    prompts={draft.prompts}
                    intervalMs={draft.intervalMs}
                    messages={messages}
                    promptRef={(index, node) => {
                      groupPromptRefs.current[index] = node;
                    }}
                    onIntervalChange={(intervalMs) => setDraft({ ...draft, intervalMs })}
                    onPromptChange={setDraftPrompt}
                    onInsertPrompt={(index) => {
                      const next = [...draft.prompts];
                      next.splice(index + 1, 0, "");
                      setDraft({ ...draft, prompts: next });
                    }}
                    onRemovePrompt={(index) => {
                      const next = draft.prompts.filter((_, i) => i !== index);
                      setDraft({ ...draft, prompts: next.length ? next : [""] });
                    }}
                    onMovePrompt={(from, to) => {
                      setDraft({ ...draft, prompts: moveArrayItem(draft.prompts, from, to) });
                    }}
                  />
                )}
                <div className="editor-submit-row">
                  <button
                    className="button button-secondary"
                    type="button"
                    onClick={cancelCreate}
                  >
                    {messages.manager.cancel}
                  </button>
                  <button
                    className="button button-primary editor-submit"
                    type="submit"
                    onPointerDown={(event) => event.preventDefault()}
                    onPointerUp={() => runSubmitOnce(() => handleCreate(draftFromCreateDom()))}
                  >
                    {draft.type === "group" ? messages.manager.addGroup : messages.manager.addPrompt}
                  </button>
                </div>
              </form>
            ) : null}
            {createToastMessage ? (
              <div className="create-toast" role="status" aria-live="polite">
                <span className="create-toast-icon" aria-hidden="true">✓</span>
                <span>{createToastMessage}</span>
              </div>
            ) : null}
            <div className="prompt-list">
              {prompts.length === 0 ? (
                <div className="empty-state-block">
                  {activeCategoryId ? messages.manager.emptyCategory : messages.manager.noPrompts}
                </div>
              ) : prompts.map((prompt, index) => (
                <div
                  key={prompt.id}
                  className={`prompt-item ${prompt.type === "group" ? "prompt-item-group" : ""}`}
                >
                  {editingId === prompt.id ? (
                    <form
                      className="edit-form edit-form-stacked"
                      onSubmit={(event) => {
                        event.preventDefault();
                        runSubmitOnce(() => handleSaveEdit(prompt.id, draftFromEditDom()));
                      }}
                    >
                      <div
                        className="segmented-control"
                        aria-label={messages.manager.editPromptContainerType}
                      >
                        <button
                          className={editDraft.type === "single" ? "is-selected" : ""}
                          type="button"
                          aria-pressed={editDraft.type === "single"}
                          onClick={() => setEditDraft({ ...editDraft, type: "single" })}
                        >
                          {messages.manager.single}
                        </button>
                        <button
                          className={editDraft.type === "group" ? "is-selected" : ""}
                          type="button"
                          aria-pressed={editDraft.type === "group"}
                          onClick={() => setEditDraft({ ...editDraft, type: "group" })}
                        >
                          {messages.manager.group}
                        </button>
                      </div>
                      <input
                        ref={editTitleInputRef}
                        className="field"
                        type="text"
                        value={editDraft.title}
                        onChange={(e) => setEditDraft({ ...editDraft, title: e.target.value })}
                      />
                      {editDraft.type === "single" ? (
                        <textarea
                          ref={editBodyTextareaRef}
                          className="field prompt-body-field"
                          value={editDraft.body}
                          onChange={(e) => setEditDraft({ ...editDraft, body: e.target.value })}
                        />
                      ) : (
                        <GroupFields
                          prompts={editDraft.prompts}
                          intervalMs={editDraft.intervalMs}
                          messages={messages}
                          promptRef={(entryIndex, node) => {
                            editGroupPromptRefs.current[entryIndex] = node;
                          }}
                          onIntervalChange={(intervalMs) =>
                            setEditDraft({ ...editDraft, intervalMs })
                          }
                          onPromptChange={setEditPrompt}
                          onInsertPrompt={(entryIndex) => {
                            const next = [...editDraft.prompts];
                            next.splice(entryIndex + 1, 0, "");
                            setEditDraft({ ...editDraft, prompts: next });
                          }}
                          onRemovePrompt={(entryIndex) => {
                            const next = editDraft.prompts.filter((_, i) => i !== entryIndex);
                            setEditDraft({ ...editDraft, prompts: next.length ? next : [""] });
                          }}
                          onMovePrompt={(from, to) => {
                            setEditDraft({
                              ...editDraft,
                              prompts: moveArrayItem(editDraft.prompts, from, to),
                            });
                          }}
                        />
                      )}
                      <div className="edit-actions">
                        <button
                          className="button button-primary"
                          type="submit"
                          onPointerDown={(event) => event.preventDefault()}
                          onPointerUp={() => runSubmitOnce(() =>
                            handleSaveEdit(prompt.id, draftFromEditDom())
                          )}
                        >
                          {messages.manager.save}
                        </button>
                        <button
                          className="button button-secondary"
                          type="button"
                          onClick={() => setEditingId(null)}
                        >
                          {messages.manager.cancel}
                        </button>
                      </div>
                    </form>
                  ) : deleteConfirmId === prompt.id ? (
                    <div className="delete-confirm">
                      <span>{messages.manager.deleteConfirm}</span>
                      <div className="confirm-actions">
                        <button
                          className="button button-danger"
                          onClick={() => { onDelete(prompt.id); setDeleteConfirmId(null); }}
                        >
                          {messages.manager.confirm}
                        </button>
                        <button
                          className="button button-secondary"
                          onClick={() => setDeleteConfirmId(null)}
                        >
                          {messages.manager.cancel}
                        </button>
                      </div>
                    </div>
                  ) : (
                    <div className="prompt-content">
                      <div className="prompt-info">
                        <div className="prompt-title-row">
                          <strong>{prompt.title}</strong>
                          <PromptKindBadge prompt={prompt} messages={messages} />
                        </div>
                        <span className="prompt-preview-lines">
                          {getPromptContainerPreviewLines(prompt).map((line) => (
                            <span className="prompt-preview-line" key={line}>
                              {line}
                            </span>
                          ))}
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
                          {messages.manager.edit}
                        </button>
                        <button
                          className="button button-ghost-danger"
                          onClick={() => setDeleteConfirmId(prompt.id)}
                        >
                          {messages.manager.delete}
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}

interface GroupFieldsProps {
  prompts: string[];
  intervalMs: number;
  messages: Messages;
  promptRef?: (index: number, node: HTMLTextAreaElement | null) => void;
  onIntervalChange: (intervalMs: number) => void;
  onPromptChange: (index: number, value: string) => void;
  onInsertPrompt: (index: number) => void;
  onRemovePrompt: (index: number) => void;
  onMovePrompt: (from: number, to: number) => void;
}

function GroupFields({
  prompts,
  intervalMs,
  messages,
  promptRef,
  onIntervalChange,
  onPromptChange,
  onInsertPrompt,
  onRemovePrompt,
  onMovePrompt,
}: GroupFieldsProps) {
  const [draggingIndex, setDraggingIndex] = useState<number | null>(null);
  const [intervalSecondsInput, setIntervalSecondsInput] = useState(() =>
    formatIntervalSeconds(intervalMs)
  );

  useEffect(() => {
    setIntervalSecondsInput(formatIntervalSeconds(intervalMs));
  }, [intervalMs]);

  const handleDragStart = (event: DragEvent<HTMLButtonElement>, index: number) => {
    setDraggingIndex(index);
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", String(index));
  };

  const handleDrop = (event: DragEvent<HTMLDivElement>, targetIndex: number) => {
    event.preventDefault();
    const sourceIndex = draggingIndex;
    setDraggingIndex(null);
    if (sourceIndex === null || sourceIndex === targetIndex) return;
    onMovePrompt(sourceIndex, targetIndex);
  };

  const handleIntervalChange = (value: string) => {
    setIntervalSecondsInput(value);
    if (!value.trim()) return;
    onIntervalChange(intervalSecondsToMs(Number(value)));
  };

  return (
    <div className="group-editor">
      <label className="interval-field">
        <span>{messages.manager.delayBetweenPrompts}</span>
        <input
          aria-label={messages.manager.delayBetweenPrompts}
          className="field"
          type="number"
          min={MIN_GROUP_INTERVAL_MS / 1000}
          max={MAX_GROUP_INTERVAL_MS / 1000}
          step={GROUP_INTERVAL_STEP_SECONDS}
          value={intervalSecondsInput}
          onBlur={() => {
            if (!intervalSecondsInput.trim()) {
              setIntervalSecondsInput(formatIntervalSeconds(intervalMs));
            }
          }}
          onChange={(e) => handleIntervalChange(e.target.value)}
        />
        <span>s</span>
      </label>
      <div className="group-prompt-list">
        {prompts.map((body, index) => (
          <div
            className={`group-prompt-row ${draggingIndex === index ? "is-dragging" : ""}`}
            key={index}
            onDragOver={(event) => event.preventDefault()}
            onDrop={(event) => handleDrop(event, index)}
          >
            <button
              aria-label={messages.manager.dragPrompt(index + 1)}
              className="group-prompt-handle"
              draggable
              type="button"
              onDragStart={(event) => handleDragStart(event, index)}
              onDragEnd={() => setDraggingIndex(null)}
            >
              <span aria-hidden="true">⋮⋮</span>
              {messages.manager.promptLabel(index + 1)}
            </button>
            <textarea
              ref={(node) => promptRef?.(index, node)}
              aria-label={messages.manager.promptBody(index + 1)}
              className="field prompt-body-field"
              value={body}
              onChange={(e) => onPromptChange(index, e.target.value)}
            />
            <div className="group-prompt-actions">
              <button
                aria-label={messages.manager.insertPromptAfter(index + 1)}
                className="button icon-button group-icon-button"
                type="button"
                onClick={() => onInsertPrompt(index)}
              >
                +
              </button>
              <button
                aria-label={messages.manager.removePrompt(index + 1)}
                className="button icon-button group-icon-button"
                type="button"
                disabled={prompts.length === 1}
                onClick={() => onRemovePrompt(index)}
              >
                -
              </button>
              <button
                aria-label={messages.manager.movePromptUp(index + 1)}
                className="button icon-button group-icon-button"
                type="button"
                disabled={index === 0}
                onClick={() => onMovePrompt(index, index - 1)}
              >
                ↑
              </button>
              <button
                aria-label={messages.manager.movePromptDown(index + 1)}
                className="button icon-button group-icon-button"
                type="button"
                disabled={index === prompts.length - 1}
                onClick={() => onMovePrompt(index, index + 1)}
              >
                ↓
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
