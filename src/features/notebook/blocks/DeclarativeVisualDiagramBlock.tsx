import type { DynamicSectionBlock } from "../../../lib/schemas";
import { InlineText } from "./RichText";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "declarative_visual_diagram" }>;

export function DeclarativeVisualDiagramBlock({ content }: { content: Content }) {
  return (
    <div className="notebook-block-body">
      <h3>
        <InlineText text={content.title} />
      </h3>
      <StaticVisual visual={content.visualAid} />
      <ol className="walkthrough-list">
        {content.guidedWalkthrough
          .slice()
          .sort((a, b) => a.stepNumber - b.stepNumber)
          .map((step) => (
            <li key={step.stepNumber}>
              <strong>
                <InlineText text={step.targetVisualElement} />:
              </strong>{" "}
              <InlineText text={step.pedagogicalInsight} />
            </li>
          ))}
      </ol>
    </div>
  );
}
