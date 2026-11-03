import { useState } from "react";
import { DynamicSectionBlockSchema, type NotebookBlock } from "../../lib/schemas";
import { DynamicNotebookBlock } from "../notebook/blocks";
import { BLOCK_CATEGORIES, type BlockPreset, type BlockTypeCategory } from "./mockPresets";

export function BlockGalleryTab() {
  const [selectedCatId, setSelectedCatId] = useState<string>("anchored_micro_theory");
  const selectedCat = BLOCK_CATEGORIES.find((c) => c.id === selectedCatId) ?? BLOCK_CATEGORIES[0]!;

  const [selectedPresetId, setSelectedPresetId] = useState<string>(selectedCat.presets[0]?.id ?? "");
  const activePreset = selectedCat.presets.find((p) => p.id === selectedPresetId) ?? selectedCat.presets[0]!;

  // Editable JSON state
  const [jsonText, setJsonText] = useState<string>(() => JSON.stringify(activePreset.block.content_json, null, 2));
  const [jsonError, setJsonError] = useState<string | null>(null);
  const [activeJsonBlock, setActiveJsonBlock] = useState<NotebookBlock>(activePreset.block);
  const [showJsonEditor, setShowJsonEditor] = useState<boolean>(false);
  const [copied, setCopied] = useState<boolean>(false);

  // Switch category
  const handleSelectCategory = (cat: BlockTypeCategory) => {
    setSelectedCatId(cat.id);
    const firstPreset = cat.presets[0]!;
    setSelectedPresetId(firstPreset.id);
    const str = JSON.stringify(firstPreset.block.content_json, null, 2);
    setJsonText(str);
    setJsonError(null);
    setActiveJsonBlock(firstPreset.block);
  };

  // Switch preset
  const handleSelectPreset = (preset: BlockPreset) => {
    setSelectedPresetId(preset.id);
    const str = JSON.stringify(preset.block.content_json, null, 2);
    setJsonText(str);
    setJsonError(null);
    setActiveJsonBlock(preset.block);
  };

  // Live JSON editor change
  const handleJsonChange = (newText: string) => {
    setJsonText(newText);
    try {
      const parsed = JSON.parse(newText);
      const schemaCheck = DynamicSectionBlockSchema.safeParse(parsed);
      if (!schemaCheck.success) {
        setJsonError(
          schemaCheck.error.issues.map((i) => `${i.path.join(".") || "(raíz)"}: ${i.message}`).join("\n")
        );
      } else {
        setJsonError(null);
        setActiveJsonBlock({
          ...activePreset.block,
          content_json: parsed,
        });
      }
    } catch (e) {
      setJsonError("Error de sintaxis JSON: " + (e as Error).message);
    }
  };

  const handleResetJson = () => {
    const str = JSON.stringify(activePreset.block.content_json, null, 2);
    setJsonText(str);
    setJsonError(null);
    setActiveJsonBlock(activePreset.block);
  };

  const handleCopyJson = () => {
    navigator.clipboard.writeText(jsonText).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  return (
    <div className="gallery-layout">
      {/* Sidebar: Block Types */}
      <aside className="gallery-sidebar" aria-label="Catálogo de bloques">
        <div className="gallery-sidebar-head">
          <h3>Bloques pedagógicos</h3>
          <span className="badge">{BLOCK_CATEGORIES.length} tipos</span>
        </div>
        <nav className="gallery-nav">
          {BLOCK_CATEGORIES.map((cat) => {
            const active = cat.id === selectedCatId;
            return (
              <button
                key={cat.id}
                type="button"
                className={`gallery-nav-item ${active ? "active" : ""}`}
                onClick={() => handleSelectCategory(cat)}
              >
                <span className="gallery-nav-icon">{cat.icon}</span>
                <span className="gallery-nav-label">{cat.label}</span>
              </button>
            );
          })}
        </nav>
      </aside>

      {/* Main Content Area */}
      <section className="gallery-main">
        {/* Header Bar */}
        <header className="gallery-header">
          <div className="gallery-header-info">
            <div className="gallery-header-title-row">
              <span className="gallery-icon-lg">{selectedCat.icon}</span>
              <h2>{selectedCat.label}</h2>
              <span className="badge">{activePreset.domain}</span>
            </div>
            <p className="gallery-role-desc">{selectedCat.pedagogicalRole}</p>
          </div>

          <div className="gallery-header-actions">
            <button
              type="button"
              className={`btn-quiet ${showJsonEditor ? "active" : ""}`}
              onClick={() => setShowJsonEditor(!showJsonEditor)}
              title="Abrir o cerrar el editor de payload JSON en vivo"
            >
              {showJsonEditor ? "Ocultar editor JSON" : "🛠 Editar JSON"}
            </button>
          </div>
        </header>

        {/* Preset Selector */}
        {selectedCat.presets.length > 1 && (
          <div className="gallery-presets-row">
            <span className="gallery-presets-label">Ejemplos disponibles:</span>
            {selectedCat.presets.map((preset) => (
              <button
                key={preset.id}
                type="button"
                className={`gallery-preset-pill ${preset.id === selectedPresetId ? "active" : ""}`}
                onClick={() => handleSelectPreset(preset)}
              >
                {preset.name}
              </button>
            ))}
          </div>
        )}

        {/* Live Block Render Zone */}
        <div className="gallery-canvas-area">
          <div className="gallery-preview-card">
            <div className="gallery-preview-banner">
              <span className="gallery-live-tag">Vista en vivo e interactiva</span>
              <span className="muted text-xs">Prueba rellenar huecos, responder o interactuar</span>
            </div>

            <div className="gallery-block-render">
              <DynamicNotebookBlock
                block={activeJsonBlock}
                isActive={true}
                lessonKeyConcepts={["Diseño", "Tokens", "Variables"]}
                onSubmitGate={async () => ({ passed: true, block: activeJsonBlock, attempts: 1, feedback: "¡Excelente! Respuesta correcta." })}
                onRequestEasierStep={async () => ({ passed: false, block: activeJsonBlock, attempts: 1, feedback: "Paso guiado activado." })}
                onSave={() => {}}
                onRequestClosureFeedback={async () => ({ passed: true, feedback: "Síntesis clara y profunda." })}
                onContinueModule={() => {}}
                continueLabel="Continuar"
              />
            </div>
          </div>

          {/* JSON Inspector / Live Editor */}
          {showJsonEditor && (
            <div className="gallery-json-panel">
              <div className="gallery-json-head">
                <span className="gallery-json-title">Payload JSON (Zod schema)</span>
                <div className="gallery-json-actions">
                  <button type="button" className="btn-quiet text-xs" onClick={handleCopyJson}>
                    {copied ? "¡Copiado!" : "Copiar JSON"}
                  </button>
                  <button type="button" className="btn-quiet text-xs" onClick={handleResetJson}>
                    Restaurar original
                  </button>
                </div>
              </div>

              {jsonError && (
                <div className="alert error text-xs" style={{ margin: "0.5rem" }}>
                  <strong>Error de esquema:</strong>
                  <pre style={{ whiteSpace: "pre-wrap", marginTop: "0.25rem" }}>{jsonError}</pre>
                </div>
              )}

              <textarea
                className="gallery-json-textarea"
                value={jsonText}
                onChange={(e) => handleJsonChange(e.target.value)}
                rows={16}
                spellCheck={false}
              />
            </div>
          )}
        </div>

        {/* Pedagogical Health & Improvement Guidelines */}
        <section className="gallery-guidelines-card">
          <h4>💡 Criterios para mejorar y auditar este bloque</h4>
          <div className="gallery-guidelines-grid">
            <div className="gallery-guideline-item">
              <strong>🎯 Carga cognitiva:</strong>
              <p>El texto debe enfocarse en un solo concepto a la vez. Evitar párrafos acumulativos sin saltos de línea.</p>
            </div>
            <div className="gallery-guideline-item">
              <strong>⚖️ Sesgo de distractores:</strong>
              <p>Las opciones de respuesta deben mantener una longitud equilibrada (variación &lt; 15% entre min y max car.).</p>
            </div>
            <div className="gallery-guideline-item">
              <strong>🔧 Feedback inmediato:</strong>
              <p>Cada interacción debe proporcionar retroalimentación constructiva que ancle el porqué del error, no solo un acierto/fallo.</p>
            </div>
          </div>
        </section>
      </section>
    </div>
  );
}
