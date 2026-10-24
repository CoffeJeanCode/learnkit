//! Learning resources attached to each class — real tutorials, documentation,
//! videos and exercises found on the web, so a class about Linux, CAD or
//! photography points at something the student can actually DO instead of
//! staying theory.
//!
//! The model never invents a resource. The pipeline is: the model proposes
//! search QUERIES; a search provider returns real HITS; the model only PICKS
//! among those hits; and [`ground_selection`] keeps a pick only when its URL is
//! one of the hits actually returned for that class — the title comes from the
//! hit, never from the model. A hallucinated link therefore cannot reach the
//! student: it is dropped before anything is stored.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Search queries kept per class.
pub const MAX_QUERIES_PER_CLASS: usize = 2;
/// Queries kept per course: bounds the number of search calls.
pub const MAX_TOTAL_QUERIES: usize = 60;
pub const MAX_QUERY_CHARS: usize = 120;
/// Resources shown per class: a short, curated list beats a link dump.
pub const MAX_RESOURCES_PER_CLASS: usize = 4;
const MAX_TITLE_CHARS: usize = 140;
const MAX_TEXT_CHARS: usize = 240;
pub const MAX_SNIPPET_CHARS: usize = 300;

/// One result from the search provider. This is the ONLY source of resource
/// titles and URLs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Tutorial,
    Documentation,
    Video,
    Exercise,
    /// Also what an unrecognized kind from the model degrades to.
    #[serde(other)]
    Reference,
}

// --- What the model returns ---------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchQuery {
    pub class_id: String,
    pub query: String,
}

/// `submit_research_plan`: what to search for, per class.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchPlanArgs {
    pub queries: Vec<ResearchQuery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePick {
    pub url: String,
    pub kind: ResourceKind,
    /// Why THIS resource helps THIS class.
    #[serde(default)]
    pub why: String,
    /// The concrete first thing to do with it ("sigue la sección «Primeros pasos»").
    #[serde(default)]
    pub what_to_do: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassPicks {
    pub class_id: String,
    pub picks: Vec<ResourcePick>,
}

/// `submit_resource_selection`: which of the found hits to keep, per class.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSelectionArgs {
    pub classes: Vec<ClassPicks>,
}

// --- What is stored and shown -------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassResource {
    /// From the search hit, never from the model.
    pub title: String,
    pub url: String,
    pub kind: ResourceKind,
    pub why: String,
    pub what_to_do: String,
}

/// Where the research for a course stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchStatus {
    Pending,
    Running,
    Done,
    /// No search key configured: nothing was attempted (not an error).
    SkippedNoKey,
    Failed,
}

fn truncate_chars(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max { s.to_string() } else { s.chars().take(max).collect::<String>().trim_end().to_string() }
}

/// Identity of a URL for matching and de-duplication: http(s) only, lowercase
/// scheme/host, no fragment, no tracking parameters, no trailing slash.
/// `None` for anything else (`javascript:`, `file:`, garbage).
pub fn normalize_url(raw: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    url.set_fragment(None);
    let kept: Vec<(String, String)> =
        url.query_pairs().filter(|(k, _)| !k.starts_with("utm_") && k != "fbclid" && k != "gclid").map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    if kept.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(kept);
    }
    let mut out = url.to_string();
    if out.ends_with('/') && url.path() != "/" {
        out.pop();
    } else if out.ends_with('/') && url.query().is_none() {
        out.pop();
    }
    Some(out)
}

/// Cleans the raw hits of one query: only http(s) links with a title, no
/// duplicates, snippets capped. Order is kept (the provider ranks them).
pub fn clean_hits(hits: Vec<SearchHit>) -> Vec<SearchHit> {
    let mut seen = HashSet::new();
    hits.into_iter()
        .filter_map(|h| {
            let normalized = normalize_url(&h.url)?;
            let title = truncate_chars(&h.title, MAX_TITLE_CHARS);
            if title.is_empty() || !seen.insert(normalized) {
                return None;
            }
            Some(SearchHit { title, url: h.url.trim().to_string(), snippet: truncate_chars(&h.snippet, MAX_SNIPPET_CHARS) })
        })
        .collect()
}

/// Keeps the queries worth running: known class, non-empty, capped in length
/// and count, no repeats. Returns them with the reasons anything was dropped
/// (for the dev trace).
pub fn sanitize_plan(args: &ResearchPlanArgs, known_class_ids: &HashSet<String>) -> (Vec<ResearchQuery>, Vec<String>) {
    let mut kept: Vec<ResearchQuery> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut per_class: HashMap<&str, usize> = HashMap::new();

    for q in &args.queries {
        let query = truncate_chars(&q.query, MAX_QUERY_CHARS);
        if !known_class_ids.contains(&q.class_id) {
            dropped.push(format!("clase desconocida: {}", q.class_id));
            continue;
        }
        if query.is_empty() {
            dropped.push(format!("consulta vacía para {}", q.class_id));
            continue;
        }
        if !seen.insert((q.class_id.clone(), query.to_lowercase())) {
            continue;
        }
        let count = per_class.entry(q.class_id.as_str()).or_insert(0);
        if *count >= MAX_QUERIES_PER_CLASS || kept.len() >= MAX_TOTAL_QUERIES {
            dropped.push(format!("de más para {}", q.class_id));
            continue;
        }
        *count += 1;
        kept.push(ResearchQuery { class_id: q.class_id.clone(), query });
    }
    (kept, dropped)
}

/// THE anti-hallucination rule. Turns the model's picks into stored resources,
/// keeping a pick only when its URL is one of the hits found for THAT class.
/// Titles come from the hit; texts are trimmed and capped; repeats and
/// anything past [`MAX_RESOURCES_PER_CLASS`] are dropped. A class with no
/// surviving pick is simply absent from the result.
pub fn ground_selection(args: &ResourceSelectionArgs, hits_by_class: &HashMap<String, Vec<SearchHit>>) -> HashMap<String, Vec<ClassResource>> {
    let mut out: HashMap<String, Vec<ClassResource>> = HashMap::new();
    for class in &args.classes {
        let Some(hits) = hits_by_class.get(&class.class_id) else { continue };
        let by_url: HashMap<String, &SearchHit> = hits.iter().filter_map(|h| normalize_url(&h.url).map(|u| (u, h))).collect();
        let mut seen: HashSet<String> = HashSet::new();
        let mut kept: Vec<ClassResource> = Vec::new();
        for pick in &class.picks {
            if kept.len() >= MAX_RESOURCES_PER_CLASS {
                break;
            }
            let Some(key) = normalize_url(&pick.url) else { continue };
            let Some(hit) = by_url.get(&key) else { continue };
            if !seen.insert(key) {
                continue;
            }
            let why = truncate_chars(&pick.why, MAX_TEXT_CHARS);
            kept.push(ClassResource {
                title: hit.title.clone(),
                url: hit.url.clone(),
                kind: pick.kind,
                // A pick without a reason still has the page's own summary to show.
                why: if why.is_empty() { truncate_chars(&hit.snippet, MAX_TEXT_CHARS) } else { why },
                what_to_do: truncate_chars(&pick.what_to_do, MAX_TEXT_CHARS),
            });
        }
        if !kept.is_empty() {
            out.entry(class.class_id.clone()).or_default().extend(kept);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(title: &str, url: &str, snippet: &str) -> SearchHit {
        SearchHit { title: title.to_string(), url: url.to_string(), snippet: snippet.to_string() }
    }

    fn pick(url: &str, why: &str) -> ResourcePick {
        ResourcePick { url: url.to_string(), kind: ResourceKind::Tutorial, why: why.to_string(), what_to_do: "Sigue la primera sección".to_string() }
    }

    fn ids(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // --- URL identity ---------------------------------------------------------

    #[test]
    fn urls_that_differ_only_in_noise_are_the_same_resource() {
        let canonical = normalize_url("https://example.com/guide").expect("valid");
        for same in [
            "https://EXAMPLE.com/guide/",
            "https://example.com/guide#install",
            "https://example.com/guide?utm_source=newsletter&utm_medium=email",
            "  https://example.com/guide  ",
            "https://example.com/guide?fbclid=abc",
        ] {
            assert_eq!(normalize_url(same).as_deref(), Some(canonical.as_str()), "{same}");
        }
        assert_ne!(normalize_url("https://example.com/guide?page=2"), Some(canonical), "a real query parameter is part of the identity");
    }

    #[test]
    fn only_web_links_are_accepted() {
        for bad in ["javascript:alert(1)", "file:///etc/passwd", "ftp://example.com/x", "data:text/html,<b>x</b>", "not a url", "", "//example.com/x"] {
            assert_eq!(normalize_url(bad), None, "{bad:?} must not become a resource");
        }
        assert!(normalize_url("http://example.com").is_some());
    }

    #[test]
    fn the_site_root_keeps_a_stable_form() {
        assert_eq!(normalize_url("https://example.com/"), normalize_url("https://example.com"));
    }

    // --- Cleaning hits --------------------------------------------------------

    #[test]
    fn hits_are_cleaned_deduplicated_and_capped() {
        let long = "x".repeat(MAX_SNIPPET_CHARS + 100);
        let cleaned = clean_hits(vec![
            hit("A", "https://a.com/x", &long),
            hit("A otra vez", "https://a.com/x/?utm_source=z", "dup"),
            hit("", "https://b.com", "sin título"),
            hit("Peligroso", "javascript:alert(1)", "nope"),
            hit("B", "https://b.com/y", "ok"),
        ]);
        assert_eq!(cleaned.iter().map(|h| h.title.as_str()).collect::<Vec<_>>(), ["A", "B"], "dup, untitled and non-web hits are gone, order kept");
        assert_eq!(cleaned[0].snippet.chars().count(), MAX_SNIPPET_CHARS);
    }

    // --- The plan -------------------------------------------------------------

    fn q(class: &str, query: &str) -> ResearchQuery {
        ResearchQuery { class_id: class.to_string(), query: query.to_string() }
    }

    #[test]
    fn the_plan_keeps_known_classes_and_caps_what_it_runs() {
        let plan = ResearchPlanArgs {
            queries: vec![
                q("c1", "instalar ubuntu paso a paso"),
                q("c1", "  INSTALAR ubuntu paso a paso "),
                q("c1", "comandos básicos de terminal"),
                q("c1", "una tercera consulta"),
                q("ghost", "de una clase que no existe"),
                q("c2", "   "),
                q("c2", &"y".repeat(500)),
            ],
        };
        let (kept, dropped) = sanitize_plan(&plan, &ids(&["c1", "c2"]));
        let c1: Vec<_> = kept.iter().filter(|k| k.class_id == "c1").map(|k| k.query.as_str()).collect();
        assert_eq!(c1, ["instalar ubuntu paso a paso", "comandos básicos de terminal"], "a repeat is dropped and at most {MAX_QUERIES_PER_CLASS} survive per class");
        let c2: Vec<_> = kept.iter().filter(|k| k.class_id == "c2").collect();
        assert_eq!(c2.len(), 1, "the blank one is gone");
        assert_eq!(c2[0].query.chars().count(), MAX_QUERY_CHARS, "an oversized query is cut, not refused");
        assert!(kept.iter().all(|k| k.class_id != "ghost"));
        assert!(dropped.iter().any(|d| d.contains("ghost")) && dropped.iter().any(|d| d.contains("vacía")), "{dropped:?}");
    }

    #[test]
    fn the_total_number_of_searches_is_bounded() {
        let many = ResearchPlanArgs { queries: (0..200).map(|i| q(&format!("c{i}"), "consulta")).collect() };
        let known: HashSet<String> = (0..200).map(|i| format!("c{i}")).collect();
        assert_eq!(sanitize_plan(&many, &known).0.len(), MAX_TOTAL_QUERIES);
    }

    // --- Grounding: the rule that stops invented links ------------------------

    fn hits_for(class: &str, hits: Vec<SearchHit>) -> HashMap<String, Vec<SearchHit>> {
        HashMap::from([(class.to_string(), hits)])
    }

    #[test]
    fn a_resource_the_model_made_up_never_reaches_the_student() {
        let hits = hits_for("c1", vec![hit("Guía real", "https://real.dev/guide", "Resumen real")]);
        let selection = ResourceSelectionArgs {
            classes: vec![ClassPicks {
                class_id: "c1".to_string(),
                picks: vec![pick("https://invented.example/tutorial-perfecto", "suena genial"), pick("https://real.dev/guide", "Cubre lo básico")],
            }],
        };
        let out = ground_selection(&selection, &hits);
        let kept = &out["c1"];
        assert_eq!(kept.len(), 1, "only the link that the search actually returned survives");
        assert_eq!(kept[0].url, "https://real.dev/guide");
    }

    #[test]
    fn the_title_comes_from_the_search_hit_not_from_the_model() {
        let hits = hits_for("c1", vec![hit("Título real de la página", "https://real.dev/guide", "x")]);
        let selection = ResourceSelectionArgs { classes: vec![ClassPicks { class_id: "c1".to_string(), picks: vec![pick("https://real.dev/guide", "porque sí")] }] };
        assert_eq!(ground_selection(&selection, &hits)["c1"][0].title, "Título real de la página");
    }

    #[test]
    fn a_link_found_for_another_class_cannot_be_borrowed() {
        let mut hits = hits_for("c1", vec![hit("Solo c1", "https://only-c1.dev", "x")]);
        hits.insert("c2".to_string(), vec![hit("Solo c2", "https://only-c2.dev", "x")]);
        let selection = ResourceSelectionArgs { classes: vec![ClassPicks { class_id: "c2".to_string(), picks: vec![pick("https://only-c1.dev", "x")] }] };
        assert!(ground_selection(&selection, &hits).is_empty(), "c2 may only pick among c2's own hits");
    }

    #[test]
    fn a_pick_matches_its_hit_through_url_noise() {
        let hits = hits_for("c1", vec![hit("Guía", "https://real.dev/guide", "x")]);
        let selection = ResourceSelectionArgs { classes: vec![ClassPicks { class_id: "c1".to_string(), picks: vec![pick("https://REAL.dev/guide/?utm_source=x#top", "x")] }] };
        let out = ground_selection(&selection, &hits);
        assert_eq!(out["c1"][0].url, "https://real.dev/guide", "the stored URL is the hit's own, not the model's spelling");
    }

    #[test]
    fn repeats_are_dropped_the_list_is_capped_and_dangerous_schemes_never_match() {
        let hits = hits_for("c1", (0..10).map(|i| hit(&format!("R{i}"), &format!("https://r{i}.dev"), "s")).collect());
        let mut picks: Vec<ResourcePick> = vec![pick("https://r0.dev", "a"), pick("https://r0.dev/", "otra vez"), pick("javascript:alert(1)", "x")];
        picks.extend((1..10).map(|i| pick(&format!("https://r{i}.dev"), "ok")));
        let out = ground_selection(&ResourceSelectionArgs { classes: vec![ClassPicks { class_id: "c1".to_string(), picks }] }, &hits);
        let kept = &out["c1"];
        assert_eq!(kept.len(), MAX_RESOURCES_PER_CLASS);
        assert_eq!(kept.iter().filter(|r| r.url == "https://r0.dev").count(), 1);
        assert!(kept.iter().all(|r| !r.url.starts_with("javascript")));
    }

    #[test]
    fn a_missing_reason_falls_back_to_the_pages_own_summary_and_texts_are_capped() {
        let hits = hits_for("c1", vec![hit("Guía", "https://real.dev/a", "Resumen de la propia página"), hit("Otra", "https://real.dev/b", "s")]);
        let long = "z".repeat(1000);
        let selection = ResourceSelectionArgs {
            classes: vec![ClassPicks {
                class_id: "c1".to_string(),
                picks: vec![
                    ResourcePick { url: "https://real.dev/a".to_string(), kind: ResourceKind::Video, why: "   ".to_string(), what_to_do: String::new() },
                    ResourcePick { url: "https://real.dev/b".to_string(), kind: ResourceKind::Exercise, why: long.clone(), what_to_do: long },
                ],
            }],
        };
        let kept = &ground_selection(&selection, &hits)["c1"];
        assert_eq!(kept[0].why, "Resumen de la propia página");
        assert_eq!(kept[0].kind, ResourceKind::Video);
        assert_eq!(kept[1].why.chars().count(), MAX_TEXT_CHARS);
        assert_eq!(kept[1].what_to_do.chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn a_class_with_no_surviving_pick_is_absent_and_an_unknown_class_is_ignored() {
        let hits = hits_for("c1", vec![hit("A", "https://a.dev", "x")]);
        let selection = ResourceSelectionArgs {
            classes: vec![
                ClassPicks { class_id: "c1".to_string(), picks: vec![pick("https://nope.dev", "x")] },
                ClassPicks { class_id: "ghost".to_string(), picks: vec![pick("https://a.dev", "x")] },
            ],
        };
        assert!(ground_selection(&selection, &hits).is_empty());
    }

    #[test]
    fn an_unrecognized_kind_from_the_model_degrades_instead_of_failing() {
        let args: ResourceSelectionArgs = serde_json::from_value(serde_json::json!({
            "classes": [{ "classId": "c1", "picks": [{ "url": "https://a.dev", "kind": "podcast", "why": "x", "whatToDo": "y" }] }]
        }))
        .expect("an odd kind must not reject the whole selection");
        assert_eq!(args.classes[0].picks[0].kind, ResourceKind::Reference);
    }

    #[test]
    fn resources_round_trip_through_json() {
        let r = ClassResource { title: "T".into(), url: "https://a.dev".into(), kind: ResourceKind::Documentation, why: "w".into(), what_to_do: "d".into() };
        let back: ClassResource = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);
        assert_eq!(serde_json::to_value(r).unwrap()["whatToDo"], "d", "camelCase for the UI");
    }
}
