import { useEffect, useState } from "react";

export function ScrollJump() {
  const [showTop, setShowTop] = useState(false);
  const [showBottom, setShowBottom] = useState(false);

  useEffect(() => {
    const workspace = document.querySelector(".workspace") as HTMLElement | null;
    const handleScroll = () => {
      const scrollY = workspace ? workspace.scrollTop : window.scrollY || document.documentElement.scrollTop;
      const scrollHeight = workspace ? workspace.scrollHeight : document.documentElement.scrollHeight;
      const clientHeight = workspace ? workspace.clientHeight : window.innerHeight || document.documentElement.clientHeight;

      // Mostrar subir si bajó más de 350px
      setShowTop(scrollY > 350);

      // Mostrar bajar si aún faltan más de 200px para el final y la página tiene scroll
      const distanceToBottom = scrollHeight - (scrollY + clientHeight);
      setShowBottom(distanceToBottom > 200 && scrollHeight > clientHeight + 400);
    };

    const target = workspace ?? window;
    target.addEventListener("scroll", handleScroll, { passive: true });
    handleScroll();
    return () => target.removeEventListener("scroll", handleScroll);
  }, []);

  const scrollToTop = () => {
    const workspace = document.querySelector(".workspace") as HTMLElement | null;
    if (workspace) {
      workspace.scrollTo({ top: 0, behavior: "smooth" });
    } else {
      window.scrollTo({ top: 0, behavior: "smooth" });
    }
  };

  const scrollToBottom = () => {
    const workspace = document.querySelector(".workspace") as HTMLElement | null;
    if (workspace) {
      workspace.scrollTo({
        top: workspace.scrollHeight,
        behavior: "smooth",
      });
    } else {
      window.scrollTo({
        top: document.documentElement.scrollHeight,
        behavior: "smooth",
      });
    }
  };

  if (!showTop && !showBottom) return null;

  return (
    <div
      aria-label="Navegación rápida de cuaderno"
      className="fixed bottom-6 right-6 z-40 flex flex-col gap-2 print:hidden"
    >
      {showTop && (
        <button
          type="button"
          onClick={scrollToTop}
          title="Ir al inicio de la clase (↑)"
          className="flex items-center justify-center w-9 h-9 rounded-full bg-slate-900/90 hover:bg-slate-800 text-slate-300 hover:text-white border border-slate-700/80 shadow-lg backdrop-blur-sm transition-all hover:scale-105 active:scale-95"
        >
          <span className="text-sm font-bold">↑</span>
        </button>
      )}
      {showBottom && (
        <button
          type="button"
          onClick={scrollToBottom}
          title="Ir al último bloque generado (↓)"
          className="flex items-center justify-center w-9 h-9 rounded-full bg-slate-900/90 hover:bg-slate-800 text-slate-300 hover:text-white border border-slate-700/80 shadow-lg backdrop-blur-sm transition-all hover:scale-105 active:scale-95"
        >
          <span className="text-sm font-bold">↓</span>
        </button>
      )}
    </div>
  );
}
