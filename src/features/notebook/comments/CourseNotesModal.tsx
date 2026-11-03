import React, { useEffect, useState } from "react";
import { useCommentsStore } from "../commentsStore";
import type { NotebookComment } from "../../../lib/schemas";
import { RichText } from "../blocks/RichText";
import { IconCheck, IconNotes, IconText } from "../../../components/Icons";

export function CourseNotesModal() {
  const { isCourseNotesOpen, courseId, setCourseNotesOpen, fetchCourseComments } = useCommentsStore();
  const [allComments, setAllComments] = useState<NotebookComment[]>([]);
  const [filter, setFilter] = useState<"all" | "note" | "doubt">("all");
  const [searchTerm, setSearchTerm] = useState("");
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (isCourseNotesOpen && courseId) {
      setLoading(true);
      fetchCourseComments(courseId)
        .then((data) => setAllComments(data))
        .finally(() => setLoading(false));
    }
  }, [isCourseNotesOpen, courseId, fetchCourseComments]);

  if (!isCourseNotesOpen) return null;

  const filtered = allComments.filter((c) => {
    if (filter === "note" && c.kind !== "note") return false;
    if (filter === "doubt" && c.kind !== "doubt") return false;
    if (searchTerm.trim()) {
      const q = searchTerm.toLowerCase();
      const inText = c.selectedText.toLowerCase().includes(q);
      const inContent = c.content.toLowerCase().includes(q);
      const inAi = c.aiResponse?.toLowerCase().includes(q) ?? false;
      const inTags = c.tags.some((t) => t.toLowerCase().includes(q));
      if (!inText && !inContent && !inAi && !inTags) return false;
    }
    return true;
  });

  // Group by classId
  const byClass = filtered.reduce<Record<string, NotebookComment[]>>((acc, c) => {
    const key = c.classId || "General";
    if (!acc[key]) acc[key] = [];
    acc[key].push(c);
    return acc;
  }, {});

  const exportMarkdown = () => {
    let md = `# Notas y Dudas del Curso\n\n`;
    for (const [classKey, items] of Object.entries(byClass)) {
      md += `## Clase: ${classKey}\n\n`;
      for (const item of items) {
        md += `### ${item.kind === "note" ? "Nota" : "Duda IA"} (${new Date(item.createdAtMs).toLocaleDateString()})\n`;
        if (item.selectedText) {
          md += `> ${item.selectedText}\n\n`;
        }
        md += `**Contenido:** ${item.content}\n\n`;
        if (item.aiResponse) {
          md += `**Respuesta IA:**\n${item.aiResponse}\n\n`;
        }
        if (item.tags.length > 0) {
          md += `*Tags:* ${item.tags.map((t) => `#${t}`).join(", ")}\n\n`;
        }
        md += `---\n\n`;
      }
    }
    navigator.clipboard.writeText(md).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2500);
    });
  };

  return (
    <div className="modal-backdrop" onClick={() => setCourseNotesOpen(false)}>
      <div className="modal-card modal-lg course-notes-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div>
            <h2>Notas de Estudio del Curso</h2>
            <p className="hint">Todas tus notas personales y dudas resueltas a lo largo de las clases.</p>
          </div>
          <div className="row gap-2">
            <button type="button" className="btn-secondary btn-sm flex items-center gap-1.5" onClick={exportMarkdown}>
              {copied ? <IconCheck /> : <IconNotes />}
              <span>{copied ? "¡Copiado!" : "Copiar Markdown"}</span>
            </button>
            <button
              type="button"
              className="btn-quiet modal-close-btn"
              onClick={() => setCourseNotesOpen(false)}
            >
              ✕
            </button>
          </div>
        </div>

        <div className="course-notes-controls row gap-2 mt-2">
          <div className="comments-filter-chips">
            <button
              type="button"
              className={`filter-chip ${filter === "all" ? "active" : ""}`}
              onClick={() => setFilter("all")}
            >
              Todas ({allComments.length})
            </button>
            <button
              type="button"
              className={`filter-chip ${filter === "note" ? "active" : ""}`}
              onClick={() => setFilter("note")}
            >
              Solo Notas ({allComments.filter((c) => c.kind === "note").length})
            </button>
            <button
              type="button"
              className={`filter-chip ${filter === "doubt" ? "active" : ""}`}
              onClick={() => setFilter("doubt")}
            >
              Solo Dudas ({allComments.filter((c) => c.kind === "doubt").length})
            </button>
          </div>
          <input
            type="search"
            className="comments-search-input flex-1"
            placeholder="Filtrar por palabra clave o #tag..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
          />
        </div>

        <div className="course-notes-content mt-4">
          {loading ? (
            <p className="hint text-center py-8">Cargando notas...</p>
          ) : Object.keys(byClass).length === 0 ? (
            <div className="comments-empty-state py-8">
              <p className="hint">No hay notas registradas para este curso aún.</p>
            </div>
          ) : (
            Object.entries(byClass).map(([classKey, items]) => (
              <div key={classKey} className="course-notes-class-section mb-6">
                <h4 className="class-section-title">Clase {classKey}</h4>
                <div className="course-notes-grid">
                  {items.map((item) => (
                    <div key={item.id} className="comment-card course-note-card">
                      <div className="comment-card-header">
                        <span className={`badge ${item.kind === "note" ? "ok" : "warn"}`}>
                          {item.kind === "note" ? "Nota" : "Duda IA"}
                        </span>
                        <span className="comment-date hint">
                          {new Date(item.createdAtMs).toLocaleDateString()}
                        </span>
                      </div>
                      {item.selectedText && (
                        <blockquote className="comment-anchor-quote">
                          "{item.selectedText}"
                        </blockquote>
                      )}
                      <p className="comment-content mt-2">{item.content}</p>
                      {item.aiResponse && (
                        <div className="comment-ai-reply mt-2">
                          <div className="comment-ai-label hint">Respuesta IA:</div>
                          <RichText text={item.aiResponse} className="comment-ai-text" />
                        </div>
                      )}
                      {item.tags.length > 0 && (
                        <div className="comment-tags mt-2">
                          {item.tags.map((tag) => (
                            <span key={tag} className="comment-tag-badge">
                              #{tag}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
