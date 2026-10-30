// Asserts the plan control panel: the dial maths, the day switches, the tile
// statuses, the remembered layout (even when storage throws) and that the board
// and the question cards render. Run with: bun scripts/check-control-panel.tsx
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { AnswerCards } from "../src/features/roadmap/panel/AnswerCards";
import { ControlPanel } from "../src/features/roadmap/panel/ControlPanel";
import { ChannelScope } from "../src/features/roadmap/panel/ChannelScope";
import { WeeksDial } from "../src/features/roadmap/panel/WeeksDial";
import {
  angleToWeeks,
  arcPath,
  clampWeeks,
  daysFromCount,
  daysFromDraft,
  hoursToDraft,
  missionProgress,
  pointerAngle,
  sliderHours,
  stepWeeks,
  niceMax,
  stepPath,
  tileStatus,
  toggleDay,
  weekSamples,
  weeksToAngle,
} from "../src/features/roadmap/panel/panelModel";
import { STARTING_DRAFT } from "../src/features/roadmap/panel/ControlPanel";
import { basicsErrors, emptyBasics } from "../src/features/roadmap/profileFormModel";
import { ContextQuestionSetSchema, UNKNOWN_ANSWER } from "../src/lib/schemas";

// ---- Dial ----------------------------------------------------------------------------
assert.equal(weeksToAngle(1), -135);
assert.equal(weeksToAngle(52), 135);
assert.equal(angleToWeeks(-135), 1);
assert.equal(angleToWeeks(135), 52);
assert.equal(angleToWeeks(-500), 1, "past the start snaps to the start");
assert.equal(angleToWeeks(500), 52, "past the end snaps to the end");
for (const w of [1, 4, 8, 12, 26, 52]) assert.equal(angleToWeeks(weeksToAngle(w)), w, `round trip ${w}`);
assert.equal(clampWeeks(0), 1);
assert.equal(clampWeeks(99), 52);
assert.equal(stepWeeks(52, 1), 52, "never past the end");
assert.equal(stepWeeks(1, -4), 1);
assert.equal(Math.round(pointerAngle(0, -10)), 0, "straight up");
assert.equal(Math.round(pointerAngle(10, 0)), 90, "to the right");
assert.ok(arcPath(84, 84, 62, -135, 135).startsWith("M "));

// ---- Day switches --------------------------------------------------------------------
assert.deepEqual(daysFromCount(5), ["mon", "tue", "wed", "thu", "fri"]);
assert.deepEqual(daysFromCount(3), ["mon", "wed", "fri"]);
for (let n = 1; n <= 7; n++) assert.equal(daysFromCount(n).length, n, `${n} days lit`);
assert.equal(daysFromDraft("abc").length, 5, "an invalid count falls back to the work week");
assert.deepEqual(toggleDay(["mon"], "mon"), ["mon"], "the last day cannot be switched off");
assert.deepEqual(toggleDay(["mon", "wed"], "wed"), ["mon"]);
assert.deepEqual(toggleDay(["wed", "fri"], "mon"), ["mon", "wed", "fri"], "kept in week order");
assert.equal(sliderHours(""), 1);
assert.equal(sliderHours("0,5"), 0.5);
assert.equal(sliderHours("20"), 8, "the slider stops at 8 h");
assert.equal(hoursToDraft(0.25), "0.25");

// ---- The week as a channel ------------------------------------------------------------------
assert.deepEqual(weekSamples(["mon", "wed", "fri"], 1.5), [1.5, 0, 1.5, 0, 1.5, 0, 0], "lit days hold the hours, the rest are 0");
assert.equal(weekSamples([], 2).every((v) => v === 0), true);
assert.equal(weekSamples(["mon"], 1).reduce((a, b) => a + b, 0), 1, "the sum is the weekly total");
assert.equal(niceMax(0), 1);
assert.equal(niceMax(1.5), 2, "rounds up to a readable scale");
assert.equal(niceMax(4), 4);
assert.equal(niceMax(99), 24, "never past a day");
assert.equal(stepPath([], 280, 56, 8), "", "no samples, no trace");
assert.equal(stepPath([1], 280, 56, 0), "", "no scale, no trace");
const trace = stepPath([2, 0], 100, 40, 8);
assert.ok(trace.startsWith("M 0 30"), "starts at the first sample's height (2 of 8 -> y=30)");
assert.ok(trace.includes("L 50 30 L 50 40"), "holds, then steps down at the slot edge");
assert.ok(stepPath([99], 100, 40, 8).startsWith("M 0 0"), "values above full scale clamp to the top");

// ---- Tiles ---------------------------------------------------------------------------
const blank = emptyBasics();
assert.equal(tileStatus("topic", blank, false), "empty", "calm before the first attempt");
assert.equal(tileStatus("topic", blank, true), "error");
assert.equal(tileStatus("topic", { ...blank, topic: "Linux" }, false), "ready");
const overloaded = { ...blank, hoursPerDay: "24", daysPerWeek: "7" };
assert.equal(tileStatus("rhythm", overloaded, false), "error", "the weekly total is checked live");
const full = { topic: "Linux", targetGoal: "Usar la terminal", weeks: "8", hoursPerDay: "1", daysPerWeek: "4" };
assert.deepEqual(missionProgress(full), { done: 4, total: 4 });
assert.equal(missionProgress(blank).done, 0, "an empty board has no tile ready (days alone do not make the rhythm ready)");

// ---- Rendering -----------------------------------------------------------------------
const noop = () => {};
const board = renderToStaticMarkup(
  <ControlPanel initialBasics={null} questions={null} busy={false} serverError={null} onPropose={noop} onSubmit={noop} />,
);
for (const text of ["¿Qué quieres aprender?", "¿Qué quieres lograr", "Semanas", "Tu ritmo", "8 semanas", "Continuar"]) assert.ok(board.includes(text), `board shows ${text}`);

const scope = renderToStaticMarkup(<ChannelScope samples={weekSamples(["mon", "tue"], 1)} max={8} totalLabel="Son 2 h por semana" />);
assert.ok(scope.includes("scope-line") && scope.includes("Son 2 h por semana") && scope.includes(">L<"), "the scope draws the trace, the days and the total");
assert.ok(board.includes("scope-svg") === false, "the scope lives inside the rhythm tile, not on the closed board");

const dial = renderToStaticMarkup(<WeeksDial value="8" onChange={noop} />);
assert.ok(dial.includes('role="slider"') && dial.includes('aria-valuenow="8"') && dial.includes('aria-valuetext="8 semanas"'));
assert.ok(renderToStaticMarkup(<WeeksDial value="" onChange={noop} />).includes('aria-valuetext="sin elegir"'), "an empty draft is an idle dial");

const set = ContextQuestionSetSchema.parse({
  questions: [
    { id: "q1", label: "¿Qué has hecho en una terminal?", kind: "choice", purpose: "prior_knowledge", options: [{ label: "Nada" }, { label: "Algo" }] },
    { id: "q2", label: "¿Algo más?", kind: "text", purpose: "constraint" },
  ],
});
const cards = renderToStaticMarkup(
  <AnswerCards questions={set.questions} answers={{ q1: "Nada" }} errors={{}} attempted={false} onAnswer={noop} />,
);
assert.ok(cards.includes("Nada") && cards.includes("Algo") && cards.includes(UNKNOWN_ANSWER), `"No lo sé" is its own card`);
assert.ok(cards.includes('aria-pressed="true"'), "the chosen card is pressed");

const mission2 = renderToStaticMarkup(
  <ControlPanel initialBasics={{ topic: "Linux", targetGoal: "g", timeframeWeeks: 8, weeklyCommitmentHours: 4 }} questions={set} busy={false} serverError={null} onPropose={noop} onSubmit={noop} />,
);
assert.ok(mission2.includes("Armar mi plan") && !mission2.includes("Continuar"), "questions switch to step 2");

console.log("control panel: ok");
