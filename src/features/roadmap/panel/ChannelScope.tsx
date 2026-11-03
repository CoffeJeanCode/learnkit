import { DAYS, stepPath } from "./panelModel";

const W = 280;
const H = 56;

/**
 * A channel viewer, like the scope of a CHOP: your week drawn as a signal —
 * one sample per weekday, holding the hours of that day. It is read-only
 * (the day switches and the hours field are the inputs) and shows two things
 * a number can't: the SHAPE of the week, and the total under the trace.
 */
export function ChannelScope({ samples, max, totalLabel }: { samples: readonly number[]; max: number; totalLabel?: string }) {
  const path = stepPath(samples, W, H, max);
  return (
    <figure className="scope" aria-label="Tu semana como señal: horas por día">
      <span className="scope-scale" aria-hidden="true">
        {String(max).replace(".", ",")} h
      </span>
      <svg className="scope-svg" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" aria-hidden="true">
        {DAYS.map((d, i) => (
          <line key={d.id} className="scope-grid" x1={(i * W) / DAYS.length} x2={(i * W) / DAYS.length} y1={0} y2={H} />
        ))}
        <line className="scope-base" x1={0} x2={W} y1={H - 0.5} y2={H - 0.5} />
        {path && <path className="scope-fill" d={`${path} L ${W} ${H} L 0 ${H} Z`} />}
        {path && <path className="scope-line" d={path} />}
      </svg>
      <div className="scope-days" aria-hidden="true">
        {DAYS.map((d, i) => (
          <span key={d.id} className={samples[i]! > 0 ? "on" : undefined} title={`${d.label}: ${String(samples[i]).replace(".", ",")} h`}>
            {d.short}
          </span>
        ))}
      </div>
      {totalLabel && <figcaption className="scope-total">{totalLabel}</figcaption>}
    </figure>
  );
}
