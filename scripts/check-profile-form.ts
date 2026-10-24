// Asserts the two-step roadmap form: step 1 refuses what the backend would refuse,
// step 2 only accepts options the server's questions offer (whatever the topic),
// and an adjustment carries only what changed. Run with: bun scripts/check-profile-form.ts
import assert from "node:assert/strict";
import {
  EXAMPLES,
  adjustDraftFrom,
  adjustmentFrom,
  answerErrors,
  basicsErrors,
  basicsToDraft,
  emptyBasics,
  profileSummaryText,
  toAnswers,
  toBasicsPayload,
  visibleQuestions,
  weeklyHours,
} from "../src/features/roadmap/profileFormModel";
import { ContextQuestionSetSchema, RoadmapSessionSchema, UNKNOWN_ANSWER } from "../src/lib/schemas";

// ---- Step 1 --------------------------------------------------------------------------
const empty = basicsErrors(emptyBasics());
for (const key of ["topic", "targetGoal", "weeks", "hoursPerDay"]) assert.ok(empty[key], `${key} is required`);
assert.ok(!empty.daysPerWeek, "days per week starts with a sensible default");
assert.equal(toBasicsPayload(emptyBasics()), null);

const valid = { topic: " Mutaciones genéticas ", targetGoal: "Explicar cómo ocurre una mutación", weeks: "6", hoursPerDay: "0,5", daysPerWeek: "7" };
assert.deepEqual(basicsErrors(valid), {});
const payload = toBasicsPayload(valid)!;
assert.deepEqual(payload, { topic: "Mutaciones genéticas", targetGoal: "Explicar cómo ocurre una mutación", timeframeWeeks: 6, weeklyCommitmentHours: 3.5 }, "trimmed; a decimal comma counts");

// The same ranges as the backend.
for (const [weeks, ok] of [["0", false], ["1", true], ["52", true], ["53", false], ["2.5", false], ["abc", false], ["", false]] as const) {
  assert.equal("weeks" in basicsErrors({ ...valid, weeks }), !ok, `weeks=${weeks}`);
}
for (const [hoursPerDay, ok] of [["0.2", false], ["0,25", true], ["1", true], ["24", true], ["25", false], ["x", false], ["", false]] as const) {
  assert.equal("hoursPerDay" in basicsErrors({ ...valid, hoursPerDay }), !ok, `hoursPerDay=${hoursPerDay}`);
}
for (const [daysPerWeek, ok] of [["0", false], ["1", true], ["7", true], ["8", false], ["2.5", false], ["", false]] as const) {
  assert.equal("daysPerWeek" in basicsErrors({ ...valid, daysPerWeek }), !ok, `daysPerWeek=${daysPerWeek}`);
}
// The learner never multiplies: the form does, and the weekly total must fit what the backend accepts.
assert.equal(weeklyHours("1", "5"), 5);
assert.equal(weeklyHours("0,3", "3"), 0.9, "no floating-point noise (0.8999999999999999)");
assert.equal(weeklyHours("", "5"), null);
assert.ok(basicsErrors({ ...valid, hoursPerDay: "0.25", daysPerWeek: "1" }).weekly, "a total under the weekly minimum is refused with its own message");
assert.ok(basicsErrors({ ...valid, hoursPerDay: "12", daysPerWeek: "7" }).weekly, "12 h x 7 days = 84 h is above the weekly maximum");

// Every example is a valid step 1 on its own, and none is privileged by topic.
for (const ex of EXAMPLES) assert.deepEqual(basicsErrors(ex.draft), {}, `${ex.label} must be valid`);
assert.ok(EXAMPLES.some((e) => /gen|biolog/i.test(e.label)), "a non-technical topic is a first-class example");

// What the server kept round-trips into an editable draft, and the bubble matches the backend's.
assert.deepEqual(basicsToDraft(payload), { topic: "Mutaciones genéticas", targetGoal: "Explicar cómo ocurre una mutación", weeks: "6", hoursPerDay: "0.7", daysPerWeek: "5" }, "the weekly total splits back into per-day inputs (5 days preferred)");
assert.deepEqual(basicsToDraft({ ...payload, weeklyCommitmentHours: 10 }), { topic: "Mutaciones genéticas", targetGoal: "Explicar cómo ocurre una mutación", weeks: "6", hoursPerDay: "2", daysPerWeek: "5" });
assert.equal(weeklyHours(basicsToDraft({ ...payload, weeklyCommitmentHours: 3.5 }).hoursPerDay, basicsToDraft({ ...payload, weeklyCommitmentHours: 3.5 }).daysPerWeek), 3.5, "splitting then multiplying returns the stored total");
assert.equal(profileSummaryText(payload), "Tema: Mutaciones genéticas · Meta: Explicar cómo ocurre una mutación · 6 semanas · 3.5 h/semana");
assert.equal(profileSummaryText(payload).includes("día"), false, "the backend only knows hours per week");
assert.ok(!/nivel/i.test(profileSummaryText(payload)), "the level is deduced, so it is never in what the learner 'said'");

// ---- Step 2: questions the SERVER generated for the topic -----------------------------
// Biology of mutations: only knowledge questions, and no equipment question at all.
const biology = ContextQuestionSetSchema.parse({
  questions: [
    { id: "adn", label: "¿Qué sabes de ADN?", kind: "choice", purpose: "prior_knowledge", options: [{ label: "Nada todavía", signal: 0 }, { label: "Lo básico", signal: 1 }, { label: "Lo manejo bien", signal: 2 }] },
    { id: "material", label: "¿Tienes microscopio?", kind: "text", purpose: "equipment" },
  ],
});
assert.deepEqual(biology.questions[1].options, [], "a text question parses without options");

assert.deepEqual(answerErrors(biology, { adn: "Lo básico" }), {}, "free text is optional");
assert.ok(answerErrors(biology, {})["q:adn"], "a choice must be answered");
assert.ok(answerErrors(biology, { adn: "Soy experto" })["q:adn"], "and be one of the options the server offered");
assert.ok(answerErrors(biology, { adn: "Lo básico", material: "x".repeat(201) })["q:material"]);

assert.deepEqual(toAnswers(biology, { adn: "Lo básico", material: "   " }), [{ questionId: "adn", answer: "Lo básico" }], "blank text is not sent");
assert.deepEqual(toAnswers(biology, { adn: " Lo básico ", material: " un microscopio ", ghost: "x" }), [
  { questionId: "adn", answer: "Lo básico" },
  { questionId: "material", answer: "un microscopio" },
], "only the server's questions are sent, trimmed");
assert.equal(toAnswers(biology, {}), null);

// ---- "No lo sé": never a dead end -------------------------------------------------------
const linux = ContextQuestionSetSchema.parse({
  questions: [
    { id: "term", label: "¿Has usado una terminal?", kind: "choice", purpose: "prior_knowledge", options: [{ label: "Nunca", signal: 0 }, { label: "Algo", signal: 1 }] },
    {
      id: "computer", label: "¿Con qué computadora practicas?", kind: "choice", purpose: "equipment",
      options: [{ label: "Windows" }, { label: "Mac" }, { label: "Linux" }],
      followUps: [{ id: "screen", label: "¿Qué ves al encender tu computadora?", kind: "choice", purpose: "equipment", options: [{ label: "El escritorio de Windows" }, { label: "Una manzana al arrancar" }] }],
    },
  ],
});
assert.deepEqual(linux.questions[0].followUps, [], "questions without follow-ups parse with an empty list (older sets too)");

// Every choice accepts "No lo sé", whatever options the model wrote.
assert.deepEqual(answerErrors(linux, { term: UNKNOWN_ANSWER, computer: "Mac" }), {});
// The follow-up appears only after "No lo sé" on its parent...
assert.deepEqual(visibleQuestions(linux, { term: "Nunca", computer: "Mac" }).map((q) => q.id), ["term", "computer"]);
assert.deepEqual(visibleQuestions(linux, { term: "Nunca", computer: UNKNOWN_ANSWER }).map((q) => q.id), ["term", "computer", "screen"]);
// ...and is then required, but may itself be "No lo sé".
assert.ok(answerErrors(linux, { term: "Nunca", computer: UNKNOWN_ANSWER })["q:screen"], "the opened follow-up must be answered");
assert.deepEqual(answerErrors(linux, { term: "Nunca", computer: UNKNOWN_ANSWER, screen: UNKNOWN_ANSWER }), {}, "not knowing twice is still fine");
assert.deepEqual(
  toAnswers(linux, { term: "Nunca", computer: UNKNOWN_ANSWER, screen: "Una manzana al arrancar" }),
  [{ questionId: "term", answer: "Nunca" }, { questionId: "computer", answer: UNKNOWN_ANSWER }, { questionId: "screen", answer: "Una manzana al arrancar" }],
);
// Changing the parent back to a real answer drops the follow-up the server would refuse.
assert.deepEqual(
  toAnswers(linux, { term: "Nunca", computer: "Linux", screen: "Una manzana al arrancar" }),
  [{ questionId: "term", answer: "Nunca" }, { questionId: "computer", answer: "Linux" }],
);

// A session saved before the intake questions existed still parses (the fields default to null).
const parsed = RoadmapSessionSchema.parse({
  session_id: "s", phase: "onboarding", status: "active", turn_count_in_phase: 0, draft: {}, learner_profile_card: null, diagnostic_summary_card: null,
  roadmap_package: null, pending_diagnostic_battery: null, proposed_plan: null, imported_course_id: null, first_class_id: null,
  last_rejection_reasons: [], messages: [], custom_title: null, created_at_ms: 0, updated_at_ms: 0,
});
assert.equal(parsed.profile_basics, null);
assert.equal(parsed.pending_context_questions, null);
assert.equal(parsed.learning_context, null);

// ---- Plan adjustment ---------------------------------------------------------------------
const current = { weeks: 8, hours: 3.5 };
assert.equal(adjustmentFrom(adjustDraftFrom(current), current).payload, null, "untouched controls send nothing");
assert.equal(weeklyHours(adjustDraftFrom(current).hoursPerDay, adjustDraftFrom(current).daysPerWeek), 3.5, "the plan's weekly total survives the per-day split");
const moreTime = adjustmentFrom({ ...adjustDraftFrom(current), hoursPerDay: "2", daysPerWeek: "5" }, current);
assert.equal(moreTime.payload?.weeklyCommitmentHours, 10, "per-day inputs are sent as a weekly total");
const changed = adjustmentFrom({ ...adjustDraftFrom(current), weeks: "10", emphasis: "more_practice", note: "  examen en junio " }, current);
assert.deepEqual(changed.payload, { timeframeWeeks: 10, weeklyCommitmentHours: null, emphasis: "more_practice", note: "examen en junio" });
assert.equal(adjustmentFrom({ ...adjustDraftFrom(current), note: "   " }, current).payload, null, "a blank note is not a change");
assert.ok(adjustmentFrom({ ...adjustDraftFrom(current), weeks: "99" }, current).errors.weeks);
assert.ok(adjustmentFrom({ ...adjustDraftFrom(current), note: "x".repeat(301) }, current).errors.note);
assert.equal(adjustmentFrom({ ...adjustDraftFrom(current), weeks: "99" }, current).payload, null, "invalid numbers never produce a payload");

console.log("profile form rules: OK");
