import type { DynamicSectionBlock } from "../../../lib/schemas";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "declarative_visual_diagram" }>;

export function DeclarativeVisualDiagramBlock({ content }: { content: Content }) {
  return (
    <div className="notebook-block-body">
      <h3>{content.title}</h3>
      <StaticVisual visual={content.visualAid} />
      <ol className="walkthrough-list">
        {content.guidedWalkthrough
          .slice()
          .sort((a, b) => a.stepNumber - b.stepNumber)
          .map((step) => (
            <li key={step.stepNumber}>
              <strong>{step.targetVisualElement}:</strong> {step.pedagogicalInsight}
            </li>
          ))}
      </ol>
    </div>
  );
}
