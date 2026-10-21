import type { NotebookBlock } from "../../lib/schemas";
import { useDev } from "../../stores/dev";

/** Per-block inspector (dev mode only): the stored JSON and, when the block
 *  fails the zod schema, exactly which fields — which otherwise only shows as
 *  the generic "unsupported block" card. */
export function DevBlockInfo({ block, issues }: { block: NotebookBlock; issues?: string[] }) {
  const enabled = useDev((s) => s.enabled);
  if (!enabled) return null;
  return (
    <details className={"dev-block" + (issues?.length ? " bad" : "")}>
      <summary>
        🛠 {block.block_type} · {block.status} · #{block.order_index}
        {issues?.length ? <span className="badge warn">{issues.length} error(es) de esquema</span> : null}
      </summary>
      {issues?.length ? (
        <ul className="dev-notes">
          {issues.map((i, n) => (
            <li key={n}>{i}</li>
          ))}
        </ul>
      ) : null}
      <pre className="dev-pre">{JSON.stringify(block.content_json, null, 2)}</pre>
    </details>
  );
}
