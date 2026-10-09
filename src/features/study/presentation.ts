// What each study version SHOWS. Presentation only: the version must never
// change teaching, grading or scheduling (see `domain::study` in Rust), so the
// flags below are the entire surface the experiment is allowed to touch.

export type StudyVariant = "gamified" | "plain";

export interface Presentation {
  /** The "Capacidades" screen and its header entry. */
  showCapabilityMap: boolean;
  /** The "Reto de transferencia" framing on the transfer mission. The
   *  mission itself exists in BOTH versions — it is what is measured. */
  showTransferFraming: boolean;
}

/** `null` = the version has not loaded yet: show the plain presentation so
 *  the gamified UI can never flash for a participant assigned `plain`. */
export function presentationFor(variant: StudyVariant | null): Presentation {
  const gamified = variant === "gamified";
  return { showCapabilityMap: gamified, showTransferFraming: gamified };
}
