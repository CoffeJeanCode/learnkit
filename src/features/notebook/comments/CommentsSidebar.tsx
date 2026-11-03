import React from "react";
import { useCommentsStore } from "../commentsStore";
import { RichText } from "../blocks/RichText";
import { IconNotes, IconTarget, IconText, IconTrash } from "../../../components/Icons";

export function CommentsSidebar() {
  const {
    comments,
    filter,
    searchQuery,
    isSidebarOpen,
    setFilter,
    setSearchQuery,
    setSidebarOpen,
    setCourseNotesOpen,
    deleteComment,
  } = useCommentsStore();

  if (!isSidebarOpen) return null;

  const notesCount = comments.filter((c) => c.kind === "note").length;
  const doubtsCount = comments.filter((c) => c.kind === "doubt").length;

  const filteredComments = comments.filter((c) => {
    if (filter === "note" && c.kind !== "note") return false;
    if (filter === "doubt" && c.kind !== "doubt") return false;
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      const inText = c.selectedText.toLowerCase().includes(q);
      const inContent = c.content.toLowerCase().includes(q);
      const inAi = c.aiResponse?.toLowerCase().includes(q) ?? false;
      const inTags = c.tags.some((t) => t.toLowerCase().includes(q));
      return inText || inContent || inAi || inTags;
    }
    return true;
  });

  const jumpToBlock = (blockId: string) => {
    const el = document.getElementById(`block-${blockId}`);
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "center" });
      el.classList.add("block-highlight");
      setTimeout(() => el.classList.remove("block-highlight"), 2000);
    }
  };

  return (
    <>
      <button type="button" className="comments-backdrop" aria-label="Cerrar panel de notas" onClick={() => setSidebarOpen(false)} />
      <aside className="comments-sidebar-panel" aria-label="Notas y dudas de la clase">
        <div className="comments-sidebar-header">
          <div className="flex items-center gap-2">
            <IconNotes className="text-amber-300" />
            <h3 className="m-0 text-sm font-semibold text-slate-200">Comentarios y Notas</h3>
          </div>
          <button
            type="button"
            className="btn-quiet btn-close"
            onClick={() => setSidebarOpen(false)}
            aria-label="Cerrar panel de notas"
            title="Cerrar panel de notas"
          >
            ✕
          </button>
        </div>

        <div className="comments-sidebar-controls">
          <div className="filter-chips">
            <button
              type="button"
              className={`filter-chip ${filter === "all" ? "active" : ""}`}
              onClick={() => setFilter("all")}
            >
              Todas ({comments.length})
            </button>
            <button
              type="button"
              className={`filter-chip ${filter === "note" ? "active" : ""}`}
              onClick={() => setFilter("note")}
            >
              Notas ({notesCount})
            </button>
            <button
              type="button"
              className={`filter-chip ${filter === "doubt" ? "active" : ""}`}
              onClick={() => setFilter("doubt")}
            >
              Dudas ({doubtsCount})
            </button>
          </div>

          <input
            type="search"
            className="comments-search-input"
            placeholder="Buscar en notas o dudas..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>

        <div className="comments-list">
          {filteredComments.length === 0 ? (
            <div className="comments-empty-state">
              <p className="hint">
                {searchQuery
                  ? "No se encontraron comentarios que coincidan con la búsqueda."
                  : "No hay comentarios aún. Selecciona cualquier fragmento de texto en la lección para guardar una nota o aclarar una duda."}
              </p>
            </div>
          ) : (
            filteredComments.map((comment) => (
              <div key={comment.id} className="comment-card">
                <div className="comment-card-header">
                  <span className={`badge ${comment.kind === "note" ? "ok" : "warn"}`}>
                    {comment.kind === "note" ? "Nota importante" : "Duda IA"}
                  </span>
                  <span className="comment-date hint">
                    {new Date(comment.createdAtMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                  </span>
                </div>

                {comment.selectedText && (
                  <blockquote className="comment-anchor-quote">
                    "{comment.selectedText.length > 120 ? `${comment.selectedText.slice(0, 120)}…` : comment.selectedText}"
                  </blockquote>
                )}

                <div className="comment-body">
                  <p className="comment-content">{comment.content}</p>

                  {comment.aiResponse && (
                    <div className="comment-ai-reply">
                      <div className="comment-ai-label hint">Respuesta IA:</div>
                      <RichText text={comment.aiResponse} className="comment-ai-text" />
                    </div>
                  )}

                  {comment.tags.length > 0 && (
                    <div className="comment-tags">
                      {comment.tags.map((tag) => (
                        <span key={tag} className="comment-tag-badge">
                          #{tag}
                        </span>
                      ))}
                    </div>
                  )}
                </div>

                <div className="comment-card-actions">
                  <button
                    type="button"
                    className="btn-quiet btn-sm flex items-center gap-1"
                    onClick={() => jumpToBlock(comment.blockId)}
                  >
                    <IconTarget />
                    <span>Ir al bloque</span>
                  </button>
                  <button
                    type="button"
                    className="btn-quiet btn-sm btn-danger flex items-center gap-1"
                    onClick={() => void deleteComment(comment.id)}
                    title="Eliminar este comentario"
                  >
                    <IconTrash />
                  </button>
                </div>
              </div>
            ))
          )}
        </div>

        <div className="comments-sidebar-footer">
          <button
            type="button"
            className="btn-secondary w-full flex items-center justify-center gap-1.5"
            onClick={() => setCourseNotesOpen(true)}
          >
            <IconNotes />
            <span>Ver todas las notas del curso →</span>
          </button>
        </div>
      </aside>
    </>
  );
}
