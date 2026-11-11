import { useEffect, useMemo, useRef, useState } from "react";
import { type Vec3, layout, stateAt, valenceIssues } from "../../../../lib/molecule";
import type { StaticVisualSpec } from "../../../../lib/schemas";
import type { MoleculeController } from "./moleculeStage";

type Spec = Extract<StaticVisualSpec, { renderEngine: "molecule_3d" }>;
type GlState = "pending" | "ready" | "unavailable";

/** A molecule (or a reaction) in ball-and-stick 3D. The model gave atoms and
 *  bonds; positions come from a force layout, and a mechanism plays step by
 *  step — each frame laid out from the previous one so atoms glide instead of
 *  jumping — with the bonds that form and break marked. */
export function Molecule3dVisual({ visual, highlightGroupId }: { visual: Spec; highlightGroupId?: string }) {
  const hostRef = useRef<HTMLDivElement>(null);
  const ctrlRef = useRef<MoleculeController | null>(null);
  const [gl, setGl] = useState<GlState>("pending");
  const [step, setStep] = useState(0);
  const [selected, setSelected] = useState<string | null>(null);
  const atoms = visual.atoms.map((a) => ({ ...a, charge: a.charge ?? 0 }));
  const steps = visual.steps.map((s) => ({ ...s, form: s.form as Array<[string, string]>, break: s.break as Array<[string, string]> }));

  // Positions for every frame, chained so each starts from the previous one.
  const frames = useMemo(() => {
    const out: Array<{ bonds: ReturnType<typeof stateAt>["bonds"]; charges: Record<string, number>; positions: Map<string, Vec3> }> = [];
    let prev: Map<string, Vec3> | undefined;
    for (let k = 0; k <= steps.length; k++) {
      const st = stateAt(atoms, visual.bonds, steps, k);
      const positions = layout(atoms, st.bonds, prev, prev ? 160 : 400);
      out.push({ ...st, positions });
      prev = new Map([...positions].map(([id, p]) => [id, [...p] as Vec3]));
    }
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify([visual.atoms, visual.bonds, visual.steps])]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let cancelled = false;
    import("./moleculeStage")
      .then(({ mountMolecule }) => {
        if (cancelled) return;
        const ctrl = mountMolecule(host, { onPick: (id) => setSelected((c) => (c === id ? null : id)) });
        if (!ctrl) return setGl("unavailable");
        ctrlRef.current = ctrl;
        setGl("ready");
      })
      .catch(() => !cancelled && setGl("unavailable"));
    return () => {
      cancelled = true;
      ctrlRef.current?.dispose();
      ctrlRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (gl !== "ready") return;
    const f = frames[step];
    const s = step > 0 ? steps[step - 1] : null;
    ctrlRef.current?.setFrame({
      atoms: atoms.map((a) => ({ ...a, charge: f.charges[a.id] ?? 0 })),
      bonds: f.bonds,
      positions: f.positions,
      forming: s?.form ?? [],
      breaking: s?.break ?? [],
      highlight: s?.highlight ?? [],
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gl, step, frames]);

  useEffect(() => {
    if (gl === "ready") ctrlRef.current?.select(selected);
  }, [gl, selected]);

  useEffect(() => {
    if (!highlightGroupId) return;
    const a = visual.atoms.find((x) => x.id === highlightGroupId || (x.label ?? null) === highlightGroupId);
    if (a) setSelected(a.id);
  }, [highlightGroupId, visual.atoms]);

  const issues = valenceIssues(atoms, frames[step].bonds, frames[step].charges);
  const current = step > 0 ? steps[step - 1] : null;
  const has3d = gl !== "unavailable";
  return (
    <figure className="static-visual scene3d molecule-3d">
      {visual.title && <strong className="scene3d-title">{visual.title}</strong>}
      <div ref={hostRef} className="scene3d-stage scene3d-stage--model" hidden={!has3d} role="img" aria-label={`Molécula con ${atoms.length} átomos`} />
      {!has3d && <p className="hint">La vista 3D no está disponible en este equipo. Abajo tienes cada paso explicado.</p>}
      {steps.length > 0 && (
        <div className="scene3d-step">
          <div className="solid-steps-bar">
            <button type="button" className="btn-quiet" disabled={step === 0} onClick={() => setStep((s) => s - 1)} aria-label="Paso anterior">
              ←
            </button>
            <span className="solid-steps-count">{step === 0 ? "Inicio" : `Paso ${step} de ${steps.length}`}</span>
            <button type="button" className="btn-quiet" disabled={step === steps.length} onClick={() => setStep((s) => s + 1)} aria-label="Paso siguiente">
              →
            </button>
          </div>
          {current ? (
            <p>
              <strong>{current.title}.</strong> {current.detail}
            </p>
          ) : (
            <p className="hint">Avanza para ver el mecanismo: en dorado los enlaces que se forman, en rojo los que se rompen.</p>
          )}
        </div>
      )}
      {issues.length > 0 && <p className="hint hint-warn">Valencia imposible en este paso: {issues.join("; ")}</p>}
      <details className="walkthrough-all" open={!has3d}>
        <summary>Ver todos los pasos</summary>
        <ol className="walkthrough-list">
          {steps.map((s, i) => (
            <li key={i}>
              <strong>{s.title}:</strong> {s.detail}
            </li>
          ))}
        </ol>
      </details>
      {visual.caption && <figcaption className="hint">{visual.caption}</figcaption>}
    </figure>
  );
}
