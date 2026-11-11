//! `instrument`: guitar chords, a progression and a strumming pattern the
//! student SEES (chord diagrams on a fretboard) and HEARS (the frontend plays
//! it with Web Audio at `bpm`). The model only names chords: their fingerings
//! come from this closed dictionary (mirrored in `src/lib/instrument.ts`),
//! so a diagram can never show a wrong shape.

pub const INSTRUMENTS: [&str; 1] = ["guitar"];

/// Open-position shapes, strings low E → high e; `x` = not played.
pub const GUITAR_CHORDS: [(&str, &str); 19] = [
    ("C", "x32010"),
    ("D", "xx0232"),
    ("E", "022100"),
    ("G", "320003"),
    ("A", "x02220"),
    ("F", "133211"),
    ("Am", "x02210"),
    ("Em", "022000"),
    ("Dm", "xx0231"),
    ("E7", "020100"),
    ("A7", "x02020"),
    ("D7", "xx0212"),
    ("G7", "320001"),
    ("C7", "x32310"),
    ("B7", "x21202"),
    ("Cmaj7", "x32000"),
    ("Fmaj7", "xx3210"),
    ("Asus2", "x02200"),
    ("Dsus4", "xx0233"),
];

pub fn chord_shape(name: &str) -> Option<&'static str> {
    GUITAR_CHORDS.iter().find(|(n, _)| *n == name.trim()).map(|(_, s)| *s)
}

pub fn instrument_violations(i: usize, instrument: &str, chords: &[String], progression: &[(String, u32)], strum: Option<&str>, bpm: u32) -> Vec<String> {
    let f = format!("bloque {i}: instrument");
    let mut v = Vec::new();
    if !INSTRUMENTS.contains(&instrument) {
        v.push(format!("{f}: instrument debe ser guitar"));
    }
    if !(1..=6).contains(&chords.len()) {
        v.push(format!("{f}: {} acordes, debe tener 1-6", chords.len()));
    }
    for c in chords {
        if chord_shape(c).is_none() {
            v.push(format!(
                "{f}: el acorde \"{c}\" no está en el diccionario ({})",
                GUITAR_CHORDS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(" ")
            ));
        }
    }
    if progression.len() > 16 {
        v.push(format!("{f}: progresión de {} compases, máximo 16", progression.len()));
    }
    for (c, beats) in progression {
        if !chords.iter().any(|x| x.trim() == c.trim()) {
            v.push(format!("{f}: la progresión usa \"{c}\", que no está en chords"));
        }
        if !(1..=8).contains(beats) {
            v.push(format!("{f}: \"{c}\" dura {beats} tiempos, debe ser 1-8"));
        }
    }
    if let Some(s) = strum {
        if !(4..=16).contains(&s.chars().count()) || !s.chars().all(|c| matches!(c, 'D' | 'U' | '-')) {
            v.push(format!("{f}: strum \"{s}\" debe tener 4-16 casillas de D (abajo), U (arriba) o - (silencio), p. ej. \"D-DU-UDU\""));
        }
    }
    if !(40..=200).contains(&bpm) {
        v.push(format!("{f}: bpm {bpm} fuera de 40-200"));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_strummed_progression_passes_and_unknown_chords_go_back() {
        let chords: Vec<String> = ["G", "C", "D"].map(String::from).to_vec();
        let prog = vec![("G".to_string(), 4), ("C".to_string(), 4), ("D".to_string(), 4)];
        assert!(instrument_violations(0, "guitar", &chords, &prog, Some("D-DU-UDU"), 80).is_empty());
        assert!(instrument_violations(0, "guitar", &["H7".into()], &[], None, 80).iter().any(|s| s.contains("diccionario")));
        assert!(instrument_violations(0, "guitar", &chords, &[("Em".into(), 4)], None, 80).iter().any(|s| s.contains("no está en chords")));
        assert!(instrument_violations(0, "guitar", &chords, &prog, Some("abajo arriba"), 80).iter().any(|s| s.contains("strum")));
    }
}
