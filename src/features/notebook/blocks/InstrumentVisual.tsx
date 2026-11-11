import { useEffect, useMemo, useRef, useState } from "react";
import { chordFrequencies, schedule, shapeOf } from "../../../lib/instrument";
import type { StaticVisualSpec } from "../../../lib/schemas";

type Spec = Extract<StaticVisualSpec, { renderEngine: "instrument" }>;

/** One plucked string (Karplus-Strong), rendered into a buffer. */
function pluck(ctx: AudioContext, freq: number, at: number, gain: number) {
  const dur = 1.6;
  const rate = ctx.sampleRate;
  const buffer = ctx.createBuffer(1, Math.floor(rate * dur), rate);
  const data = buffer.getChannelData(0);
  const period = Math.max(2, Math.round(rate / freq));
  const ring = new Float32Array(period).map(() => Math.random() * 2 - 1);
  for (let i = 0; i < data.length; i++) {
    const j = i % period;
    const next = (j + 1) % period;
    data[i] = ring[j];
    ring[j] = 0.996 * 0.5 * (ring[j] + ring[next]);
  }
  const src = ctx.createBufferSource();
  src.buffer = buffer;
  const g = ctx.createGain();
  g.gain.value = gain;
  src.connect(g).connect(ctx.destination);
  src.start(at);
}

function click(ctx: AudioContext, at: number, accent: boolean) {
  const osc = ctx.createOscillator();
  const g = ctx.createGain();
  osc.frequency.value = accent ? 1500 : 1000;
  g.gain.setValueAtTime(0.12, at);
  g.gain.exponentialRampToValueAtTime(0.001, at + 0.05);
  osc.connect(g).connect(ctx.destination);
  osc.start(at);
  osc.stop(at + 0.06);
}

/** A chord diagram: 6 strings, 4 frets, dots where the fingers go. */
function ChordDiagram({ chord, active }: { chord: string; active: boolean }) {
  const shape = shapeOf(chord);
  if (!shape) return null;
  const frets = Math.max(4, ...shape.filter((f): f is number => f !== null));
  const x = (s: number) => 14 + s * 16;
  const y = (f: number) => 32 + f * 18;
  return (
    <svg viewBox="0 0 110 130" className={`chord-diagram${active ? " is-active" : ""}`} role="img" aria-label={`Acorde ${chord}: ${shape.map((f) => (f === null ? "x" : f)).join(" ")}`}>
      <text x="55" y="13" textAnchor="middle" className="chord-name">
        {chord}
      </text>
      <line x1={x(0)} y1={y(0)} x2={x(5)} y2={y(0)} stroke="#e2e8f0" strokeWidth="3" />
      {Array.from({ length: frets }, (_, f) => (
        <line key={f} x1={x(0)} y1={y(f + 1)} x2={x(5)} y2={y(f + 1)} stroke="#475569" />
      ))}
      {shape.map((_, s) => (
        <line key={s} x1={x(s)} y1={y(0)} x2={x(s)} y2={y(frets)} stroke="#94a3b8" />
      ))}
      {shape.map((f, s) =>
        f === null ? (
          <text key={s} x={x(s)} y={y(0) - 4} textAnchor="middle" fontSize="9" fill="#f87171">
            ×
          </text>
        ) : f === 0 ? (
          <circle key={s} cx={x(s)} cy={y(0) - 7} r="3.5" fill="none" stroke="#4ade80" />
        ) : (
          <circle key={s} cx={x(s)} cy={y(f) - 9} r="5.5" fill={active ? "#fbbf24" : "#38bdf8"} />
        ),
      )}
    </svg>
  );
}

/** Guitar chords the student sees AND hears: diagrams from a closed
 *  dictionary, a progression with its strum played by Web Audio at the
 *  lesson's tempo, a metronome, and the bar that sounds highlighted. Sound
 *  starts only on the student's tap (autoplay rules, and good manners). */
export function InstrumentVisual({ visual }: { visual: Spec }) {
  const progression = visual.progression.length ? visual.progression : visual.chords.map((c) => ({ chord: c, beats: 4 }));
  const [bpm, setBpm] = useState(visual.bpm);
  const [playing, setPlaying] = useState(false);
  const [bar, setBar] = useState<number | null>(null);
  const [metronome, setMetronome] = useState(true);
  const ctxRef = useRef<AudioContext | null>(null);
  const timers = useRef<number[]>([]);
  const plan = useMemo(() => schedule(progression, visual.strum, bpm), [progression, visual.strum, bpm]);

  const stop = () => {
    timers.current.forEach((t) => window.clearTimeout(t));
    timers.current = [];
    ctxRef.current?.close();
    ctxRef.current = null;
    setPlaying(false);
    setBar(null);
  };
  useEffect(() => stop, []);

  const play = () => {
    stop();
    const AC = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AC) return;
    const ctx = new AC();
    ctxRef.current = ctx;
    const t0 = ctx.currentTime + 0.1;
    for (const ev of plan.events) {
      const freqs = chordFrequencies(ev.chord);
      const order = ev.dir === "D" ? freqs : [...freqs].reverse();
      order.forEach((f, i) => pluck(ctx, f, t0 + ev.at + i * 0.012, ev.dir === "D" ? 0.22 : 0.15));
    }
    if (metronome) {
      const beat = 60 / bpm;
      for (let b = 0; b * beat < plan.total; b++) click(ctx, t0 + b * beat, b % 4 === 0);
    }
    let start = 0;
    progression.forEach((p, i) => {
      timers.current.push(window.setTimeout(() => setBar(i), (0.1 + start) * 1000));
      start += (p.beats * 60) / bpm;
    });
    timers.current.push(window.setTimeout(stop, (0.1 + plan.total + 1.2) * 1000));
    setPlaying(true);
  };

  const current = bar !== null ? progression[bar]?.chord : null;
  return (
    <figure className="static-visual instrument-visual">
      {visual.title && <strong className="scene3d-title">{visual.title}</strong>}
      <div className="chord-row">
        {visual.chords.map((c) => (
          <ChordDiagram key={c} chord={c} active={c === current} />
        ))}
      </div>
      <ol className="progression" aria-label="Progresión">
        {progression.map((p, i) => (
          <li key={i} className={i === bar ? "is-current" : undefined}>
            <strong>{p.chord}</strong>
            <span>{p.beats} t</span>
          </li>
        ))}
      </ol>
      {visual.strum && (
        <p className="strum" aria-label={`Rasgueo ${visual.strum}`}>
          {[...visual.strum].map((c, i) => (
            <span key={i} className={`strum-slot strum-${c === "D" ? "down" : c === "U" ? "up" : "rest"}`}>
              {c === "D" ? "↓" : c === "U" ? "↑" : "·"}
            </span>
          ))}
        </p>
      )}
      <div className="scene3d-controls">
        <button type="button" className="btn-quiet" onClick={() => (playing ? stop() : play())}>
          {playing ? "Detener" : "▶ Escuchar"}
        </button>
        <button type="button" className="btn-quiet" aria-pressed={metronome} onClick={() => setMetronome((m) => !m)}>
          {metronome ? "Metrónomo: sí" : "Metrónomo: no"}
        </button>
        <label className="im-slider instrument-bpm">
          <span className="im-slider-head">
            <span>Tempo</span>
            <output>{bpm} bpm</output>
          </span>
          <input type="range" min={40} max={200} step={2} value={bpm} disabled={playing} onChange={(e) => setBpm(Number(e.target.value))} aria-label="Tempo" />
        </label>
      </div>
      {visual.caption && <figcaption className="hint">{visual.caption}</figcaption>}
    </figure>
  );
}
