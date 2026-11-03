import { BlockGalleryTab } from "./BlockGalleryTab";
import { useUi } from "../../stores/ui";
import { IconArrowLeft, IconGallery } from "../../components/Icons";

export function GalleryStudioView() {
  const setView = useUi((s) => s.setView);
  const lastMainView = useUi((s) => s.lastMainView);

  return (
    <div className="gallery-studio-container">
      {/* Notebook-styled Header */}
      <nav className="gallery-studio-topbar" aria-label="Galería de Bloques">
        <div className="flex items-center gap-2">
          <IconGallery className="text-amber-300 w-4 h-4" />
          <h2 className="text-sm font-semibold tracking-wide uppercase text-slate-200 m-0">
            Galería de Bloques Pedagógicos
          </h2>
        </div>

        <button
          type="button"
          className="btn-quiet gallery-studio-close flex items-center gap-1.5 text-xs text-slate-300 hover:text-white"
          onClick={() => setView(lastMainView)}
          title="Regresar a la sesión o cuaderno"
        >
          <IconArrowLeft />
          <span>Volver al cuaderno</span>
        </button>
      </nav>

      {/* Gallery Content */}
      <div className="gallery-studio-content">
        <BlockGalleryTab />
      </div>
    </div>
  );
}
