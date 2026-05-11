//! Thin local learning probe for post-insertion edits.
//!
//! This records the evidence and distills it into lightweight speech skills.
//! Skills are applied only on later dictations, so learning never blocks the
//! current insertion path.

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::edit_monitor::EditMonitorSession;

const DEFAULT_LEARNING_FILE: &str = "opentypeless-learning-candidates.jsonl";
const DEFAULT_SKILL_FILE: &str = "opentypeless-speech-skills.json";
const MAX_MEDIUM_CONFIDENCE_SPAN_CHARS: usize = 80;
const MAX_SKILLS: usize = 200;
const MAX_SKILLS_FOR_PROMPT: usize = 8;

static JSONL_WRITE_LOCK: Mutex<()> = Mutex::new(());
static SKILL_WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechSkill {
    pub id: String,
    pub trigger: String,
    pub correction: String,
    pub context: Vec<String>,
    pub kind: String,
    pub confidence: String,
    pub evidence_count: u32,
    pub last_evidence: SkillEvidence,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEvidence {
    pub original_text: String,
    pub inserted_text: String,
    pub initial_field_value: String,
    pub final_text: String,
    pub from: String,
    pub to: String,
    pub target_pid: i32,
}

#[derive(Debug, Clone)]
struct SpeechSkillDraft {
    trigger: String,
    correction: String,
    context: Vec<String>,
    kind: &'static str,
    evidence: SkillEvidence,
}

pub fn record_final_candidate(
    session: &EditMonitorSession,
    target_pid: i32,
    initial_field_value: &str,
    final_field_value: &str,
) {
    if !enabled() {
        return;
    }

    let Some((payload, skill_drafts)) = build_candidate_payload(
        &session.original_text,
        &session.inserted_text,
        initial_field_value,
        final_field_value,
        target_pid,
        timestamp_ms(),
    ) else {
        log::debug!("[learning-probe] no correction candidate");
        return;
    };

    write_jsonl(payload);
    upsert_speech_skills(skill_drafts);
}

fn build_candidate_payload(
    original_text: &str,
    inserted_text: &str,
    initial_field_value: &str,
    final_text: &str,
    target_pid: i32,
    timestamp_ms: u128,
) -> Option<(Value, Vec<SpeechSkillDraft>)> {
    let correction = extract_correction(initial_field_value, final_text)?;
    let skill_drafts = build_speech_skill_drafts(
        &correction,
        original_text,
        inserted_text,
        initial_field_value,
        final_text,
        target_pid,
    );

    Some((
        json!({
            "event": "correction_candidate",
            "timestampMs": timestamp_ms,
            "source": "edit_monitor_final",
            "status": "candidate",
            "targetPid": target_pid,
            "originalText": original_text,
            "insertedText": inserted_text,
            "initialFieldValue": initial_field_value,
            "finalText": final_text,
            "correction": {
                "kind": correction.kind,
                "from": correction.from,
                "to": correction.to,
                "prefixChars": correction.prefix_chars,
                "suffixChars": correction.suffix_chars,
                "fromChars": correction.from_chars,
                "toChars": correction.to_chars,
                "confidence": correction.confidence,
            },
        }),
        skill_drafts,
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Correction {
    kind: &'static str,
    from: String,
    to: String,
    prefix_chars: usize,
    suffix_chars: usize,
    from_chars: usize,
    to_chars: usize,
    confidence: &'static str,
}

fn extract_correction(inserted_text: &str, final_text: &str) -> Option<Correction> {
    if inserted_text == final_text {
        return None;
    }

    if inserted_text.trim().is_empty() || final_text.trim().is_empty() {
        return None;
    }

    let inserted: Vec<char> = inserted_text.chars().collect();
    let final_chars: Vec<char> = final_text.chars().collect();

    let prefix_chars = common_prefix_chars(&inserted, &final_chars);
    let suffix_chars = common_suffix_chars(&inserted, &final_chars, prefix_chars);

    let inserted_end = inserted.len().saturating_sub(suffix_chars);
    let final_end = final_chars.len().saturating_sub(suffix_chars);
    let from: String = inserted[prefix_chars..inserted_end].iter().collect();
    let to: String = final_chars[prefix_chars..final_end].iter().collect();

    if from.trim().is_empty() && to.trim().is_empty() {
        return None;
    }

    let kind = match (from.is_empty(), to.is_empty()) {
        (true, false) => "insertion",
        (false, true) => "deletion",
        (false, false) => "replacement",
        (true, true) => return None,
    };
    let from_chars = from.chars().count();
    let to_chars = to.chars().count();
    let confidence = confidence_label(
        inserted.len(),
        final_chars.len(),
        prefix_chars,
        suffix_chars,
        from_chars,
        to_chars,
    );

    Some(Correction {
        kind,
        from,
        to,
        prefix_chars,
        suffix_chars,
        from_chars,
        to_chars,
        confidence,
    })
}

fn build_speech_skill_drafts(
    correction: &Correction,
    original_text: &str,
    inserted_text: &str,
    initial_field_value: &str,
    final_text: &str,
    target_pid: i32,
) -> Vec<SpeechSkillDraft> {
    if correction.kind == "deletion" {
        return Vec::new();
    }

    let context = context_terms(&[
        original_text,
        inserted_text,
        initial_field_value,
        final_text,
        &correction.from,
        &correction.to,
    ]);
    let evidence = SkillEvidence {
        original_text: original_text.to_string(),
        inserted_text: inserted_text.to_string(),
        initial_field_value: initial_field_value.to_string(),
        final_text: final_text.to_string(),
        from: correction.from.clone(),
        to: correction.to.clone(),
        target_pid,
    };
    let (skill_from, skill_to) = expanded_skill_window(initial_field_value, final_text, correction);
    extract_skill_pairs(&skill_from, &skill_to)
        .into_iter()
        .map(|(trigger, correction, kind)| SpeechSkillDraft {
            trigger,
            correction,
            context: context.clone(),
            kind,
            evidence: evidence.clone(),
        })
        .collect()
}

fn expanded_skill_window(
    from_text: &str,
    to_text: &str,
    correction: &Correction,
) -> (String, String) {
    let from_chars: Vec<char> = from_text.chars().collect();
    let to_chars: Vec<char> = to_text.chars().collect();
    let mut from_start = correction.prefix_chars.min(from_chars.len());
    let mut to_start = correction.prefix_chars.min(to_chars.len());
    let mut from_end = from_chars
        .len()
        .saturating_sub(correction.suffix_chars)
        .max(from_start);
    let mut to_end = to_chars
        .len()
        .saturating_sub(correction.suffix_chars)
        .max(to_start);

    expand_start_for_skill(&from_chars, &mut from_start);
    expand_start_for_skill(&to_chars, &mut to_start);
    expand_end_for_skill(&from_chars, &mut from_end);
    expand_end_for_skill(&to_chars, &mut to_end);

    (
        from_chars[from_start..from_end].iter().collect(),
        to_chars[to_start..to_end].iter().collect(),
    )
}

fn expand_start_for_skill(chars: &[char], start: &mut usize) {
    while *start > 0 && is_ascii_skill_char(chars[*start - 1]) {
        *start -= 1;
    }
    if *start > 0 && is_cjk(chars[*start - 1]) {
        *start -= 1;
    }
}

fn expand_end_for_skill(chars: &[char], end: &mut usize) {
    while *end < chars.len() && is_ascii_skill_char(chars[*end]) {
        *end += 1;
    }
    if *end < chars.len() && is_cjk(chars[*end]) {
        *end += 1;
    }
}

fn is_ascii_skill_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || ch.is_ascii_whitespace()
        || matches!(ch, '-' | '_' | '.' | '/' | '#' | '+')
}

fn extract_skill_pairs(from: &str, to: &str) -> Vec<(String, String, &'static str)> {
    let mut seen = HashSet::new();
    let mut pairs = Vec::new();

    let from_terms = ascii_terms_for_trigger(from);
    let to_terms = ascii_terms(to);
    let ambiguous_triggers = ambiguous_ascii_triggers(&from_terms, &to_terms);
    if !ambiguous_triggers.is_empty() {
        if let Some((trigger, correction)) = ascii_phrase_pair(from, to) {
            let key = format!("{}=>{}", trigger.to_lowercase(), correction.to_lowercase());
            if seen.insert(key) {
                pairs.push((trigger, correction, "english_phrase"));
            }
        }
    }
    for (idx, correction) in to_terms.iter().enumerate() {
        let trigger = from_terms
            .get(idx)
            .cloned()
            .or_else(|| generic_ascii_trigger(from, correction));
        let Some(trigger) = trigger else {
            continue;
        };
        if ambiguous_triggers.contains(&trigger.to_lowercase()) {
            continue;
        }
        if trigger.eq_ignore_ascii_case(correction) {
            continue;
        }
        let key = format!("{}=>{}", trigger.to_lowercase(), correction.to_lowercase());
        if seen.insert(key) {
            pairs.push((trigger, correction.clone(), "english_term"));
        }
    }

    let from_phrases = chinese_bigrams_for_trigger(from);
    for correction in chinese_bigrams(to, from) {
        let Some(trigger) = closest_chinese_trigger(&from_phrases, &correction) else {
            continue;
        };
        if trigger == correction {
            continue;
        }
        let key = format!("{trigger}=>{correction}");
        if seen.insert(key) {
            pairs.push((trigger, correction, "chinese_phrase"));
        }
    }

    pairs
}

fn ambiguous_ascii_triggers(from_terms: &[String], to_terms: &[String]) -> HashSet<String> {
    let mut mappings: HashMap<String, HashSet<String>> = HashMap::new();
    for (idx, trigger) in from_terms.iter().enumerate() {
        let Some(correction) = to_terms.get(idx) else {
            continue;
        };
        mappings
            .entry(trigger.to_lowercase())
            .or_default()
            .insert(correction.to_lowercase());
    }
    mappings
        .into_iter()
        .filter_map(|(trigger, corrections)| (corrections.len() > 1).then_some(trigger))
        .collect()
}

fn ascii_phrase_pair(from: &str, to: &str) -> Option<(String, String)> {
    let trigger = trim_skill_phrase(from);
    let correction = trim_skill_phrase(to);
    if trigger.is_empty() || correction.is_empty() || trigger.eq_ignore_ascii_case(&correction) {
        return None;
    }
    Some((trigger, correction))
}

fn trim_skill_phrase(text: &str) -> String {
    let phrase = text
        .trim()
        .trim_matches(|ch: char| {
            ch.is_ascii_punctuation()
                || matches!(ch, '。' | '，' | '、' | '；' | '：' | '！' | '？')
        })
        .trim();
    let chars: Vec<char> = phrase.chars().collect();
    let Some(start) = chars.iter().position(|ch| ch.is_ascii_alphanumeric()) else {
        return String::new();
    };
    let end = chars
        .iter()
        .rposition(|ch| ch.is_ascii_alphanumeric())
        .map(|idx| idx + 1)
        .unwrap_or(start);
    chars[start..end]
        .iter()
        .collect::<String>()
        .trim()
        .to_string()
}

fn ascii_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | '#' | '+') {
            current.push(ch);
        } else if ch.is_ascii_whitespace() && !current.trim().is_empty() {
            current.push(' ');
        } else {
            push_ascii_term(&mut terms, &mut current);
        }
    }
    push_ascii_term(&mut terms, &mut current);
    terms
}

fn ascii_terms_for_trigger(text: &str) -> Vec<String> {
    ascii_term_runs(text)
        .into_iter()
        .filter(|term| is_useful_ascii_trigger(term))
        .collect()
}

fn ascii_term_runs(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | '#' | '+') {
            current.push(ch);
        } else if ch.is_ascii_whitespace() && !current.trim().is_empty() {
            current.push(' ');
        } else {
            let term = current.trim();
            if !term.is_empty() {
                terms.push(term.to_string());
            }
            current.clear();
        }
    }
    let term = current.trim();
    if !term.is_empty() {
        terms.push(term.to_string());
    }
    terms
}

fn push_ascii_term(terms: &mut Vec<String>, current: &mut String) {
    let term = current.trim();
    if !term.is_empty() && is_useful_ascii_term(term) {
        terms.push(term.to_string());
    }
    current.clear();
}

fn is_useful_ascii_term(term: &str) -> bool {
    let normalized = term.to_lowercase();
    if normalized.len() < 3 || normalized.len() > 40 {
        return false;
    }
    if matches!(
        normalized.as_str(),
        "and" | "the" | "for" | "with" | "code" | "http"
    ) {
        return false;
    }
    normalized.chars().any(|ch| ch.is_ascii_alphabetic())
}

fn is_useful_ascii_trigger(term: &str) -> bool {
    let normalized = term.to_lowercase();
    if normalized.len() < 3 || normalized.len() > 40 {
        return false;
    }
    if matches!(normalized.as_str(), "and" | "the" | "for" | "with" | "http") {
        return false;
    }
    normalized.chars().any(|ch| ch.is_ascii_alphabetic())
}

fn generic_ascii_trigger(from: &str, correction: &str) -> Option<String> {
    let normalized = correction.to_lowercase();
    if normalized == "codex" && contains_ascii_word(from, "code") {
        return Some("code".to_string());
    }
    None
}

fn contains_ascii_word(text: &str, word: &str) -> bool {
    ascii_terms_for_trigger(text)
        .into_iter()
        .any(|term| term.eq_ignore_ascii_case(word))
}

fn chinese_bigrams(to: &str, from: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let chars: Vec<char> = to.chars().collect();
    for window in chars.windows(2) {
        if !window.iter().all(|ch| is_cjk(*ch)) {
            continue;
        }
        if window.iter().any(|ch| is_low_signal_chinese_char(*ch)) {
            continue;
        }
        let phrase: String = window.iter().collect();
        if !from.contains(&phrase) {
            candidates.push(phrase);
        }
    }
    candidates
}

fn chinese_bigrams_for_trigger(text: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    for window in chars.windows(2) {
        if !window.iter().all(|ch| is_cjk(*ch)) {
            continue;
        }
        if window.iter().any(|ch| is_low_signal_chinese_char(*ch)) {
            continue;
        }
        candidates.push(window.iter().collect());
    }
    candidates
}

fn closest_chinese_trigger(from_phrases: &[String], correction: &str) -> Option<String> {
    let correction_chars: Vec<char> = correction.chars().collect();
    from_phrases
        .iter()
        .find(|phrase| {
            let phrase_chars: Vec<char> = phrase.chars().collect();
            phrase_chars
                .iter()
                .zip(correction_chars.iter())
                .filter(|(a, b)| a == b)
                .count()
                > 0
        })
        .cloned()
}

fn context_terms(parts: &[&str]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for part in parts {
        for term in ascii_terms_for_trigger(part) {
            let key = term.to_lowercase();
            if seen.insert(key) {
                terms.push(term);
            }
        }
        for term in chinese_bigrams_for_trigger(part) {
            if seen.insert(term.clone()) {
                terms.push(term);
            }
        }
    }
    terms.truncate(12);
    terms
}

fn is_cjk(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch)
}

fn is_low_signal_chinese_char(ch: char) -> bool {
    matches!(
        ch,
        '我' | '你'
            | '他'
            | '她'
            | '它'
            | '们'
            | '想'
            | '说'
            | '的'
            | '了'
            | '是'
            | '在'
            | '和'
            | '或'
            | '不'
            | '要'
            | '就'
            | '也'
            | '都'
            | '很'
            | '更'
            | '这'
            | '那'
            | '个'
    )
}

fn common_prefix_chars(left: &[char], right: &[char]) -> usize {
    left.iter()
        .zip(right.iter())
        .take_while(|(a, b)| a == b)
        .count()
}

fn common_suffix_chars(left: &[char], right: &[char], prefix_chars: usize) -> usize {
    let max_suffix = left.len().min(right.len()).saturating_sub(prefix_chars);
    left.iter()
        .rev()
        .zip(right.iter().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count()
}

fn confidence_label(
    initial_chars: usize,
    final_chars: usize,
    prefix_chars: usize,
    suffix_chars: usize,
    from_chars: usize,
    to_chars: usize,
) -> &'static str {
    let shared_context = prefix_chars + suffix_chars;
    let changed_span = from_chars + to_chars;
    let comparable_lengths = initial_chars.max(final_chars) <= initial_chars.min(final_chars) * 3;

    if comparable_lengths && shared_context > 0 && changed_span <= MAX_MEDIUM_CONFIDENCE_SPAN_CHARS
    {
        "medium"
    } else {
        "low"
    }
}

fn write_jsonl(payload: Value) {
    let Some(path) = jsonl_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let _guard = JSONL_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(mut file) => {
            let line = format!("{payload}\n");
            match file.write_all(line.as_bytes()) {
                Ok(()) => log::info!(
                    "[learning-probe] wrote correction candidate to {}",
                    path.display()
                ),
                Err(err) => {
                    log::debug!("[learning-probe] failed to write correction candidate: {err}")
                }
            }
        }
        Err(err) => {
            log::debug!("[learning-probe] failed to write correction candidate: {err}");
        }
    }
}

pub fn speech_skill_prompt_block(raw_text: &str) -> Option<String> {
    let skills = retrieve_speech_skills(raw_text, MAX_SKILLS_FOR_PROMPT).ok()?;
    if skills.is_empty() {
        return None;
    }

    let mut lines = vec![
        "# 用户语音纠错 Skill".to_string(),
        "以下是从用户历史改写中无感学习到的语音理解偏好。只在上下文吻合时使用，不要机械替换。"
            .to_string(),
    ];
    for skill in skills {
        let context = if skill.context.is_empty() {
            "上下文相似时".to_string()
        } else {
            format!("相关上下文：{}", skill.context.join("、"))
        };
        lines.push(format!(
            "- 历史中用户曾把“{}”改成“{}”。{}，优先输出“{}”。",
            skill.trigger, skill.correction, context, skill.correction
        ));
    }
    Some(lines.join("\n"))
}

fn retrieve_speech_skills(raw_text: &str, limit: usize) -> anyhow::Result<Vec<SpeechSkill>> {
    let _guard = SKILL_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut scored: Vec<(i32, SpeechSkill)> =
        filter_ambiguous_stored_skills(read_skills_unlocked()?)
            .into_iter()
            .filter_map(|skill| {
                let score = skill_score(&skill, raw_text);
                (score > 0).then_some((score, skill))
            })
            .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.updated_at.cmp(&a.1.updated_at))
    });
    Ok(scored
        .into_iter()
        .take(limit)
        .map(|(_, skill)| skill)
        .collect())
}

fn filter_ambiguous_stored_skills(skills: Vec<SpeechSkill>) -> Vec<SpeechSkill> {
    let mut mappings: HashMap<String, HashSet<String>> = HashMap::new();
    for skill in &skills {
        if skill.kind != "english_term" {
            continue;
        }
        mappings
            .entry(skill.trigger.to_lowercase())
            .or_default()
            .insert(skill.correction.to_lowercase());
    }
    let ambiguous: HashSet<String> = mappings
        .into_iter()
        .filter_map(|(trigger, corrections)| (corrections.len() > 1).then_some(trigger))
        .collect();

    skills
        .into_iter()
        .filter(|skill| {
            skill.kind != "english_term" || !ambiguous.contains(&skill.trigger.to_lowercase())
        })
        .collect()
}

fn upsert_speech_skills(drafts: Vec<SpeechSkillDraft>) {
    if drafts.is_empty() {
        return;
    }
    let _guard = SKILL_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut existing = match read_skills_unlocked() {
        Ok(skills) => skills,
        Err(err) => {
            log::debug!("[learning-probe] failed to read speech skills: {err}");
            Vec::new()
        }
    };
    let now = Utc::now().to_rfc3339();
    for draft in drafts {
        if draft.trigger.trim().is_empty() || draft.correction.trim().is_empty() {
            continue;
        }
        if let Some(skill) = existing.iter_mut().find(|skill| {
            skill.trigger.trim().eq_ignore_ascii_case(&draft.trigger)
                && skill
                    .correction
                    .trim()
                    .eq_ignore_ascii_case(&draft.correction)
        }) {
            skill.correction = draft.correction;
            skill.kind = draft.kind.to_string();
            skill.evidence_count = skill.evidence_count.saturating_add(1);
            skill.confidence = promoted_confidence(&skill.confidence, skill.evidence_count);
            merge_context(&mut skill.context, draft.context);
            skill.updated_at = now.clone();
            skill.last_evidence = draft.evidence;
            continue;
        }

        existing.insert(
            0,
            SpeechSkill {
                id: Uuid::new_v4().to_string(),
                trigger: draft.trigger,
                correction: draft.correction,
                context: draft.context,
                kind: draft.kind.to_string(),
                confidence: "low".to_string(),
                evidence_count: 1,
                last_evidence: draft.evidence,
                created_at: now.clone(),
                updated_at: now.clone(),
            },
        );
    }
    existing.truncate(MAX_SKILLS);
    if let Err(err) = write_skills_unlocked(&existing) {
        log::debug!("[learning-probe] failed to write speech skills: {err}");
    }
}

fn read_skills_unlocked() -> anyhow::Result<Vec<SpeechSkill>> {
    let Some(path) = skill_path() else {
        return Ok(Vec::new());
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(&path)?;
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_skills_unlocked(skills: &[SpeechSkill]) -> anyhow::Result<()> {
    let Some(path) = skill_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    let json = serde_json::to_vec_pretty(skills)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

fn merge_context(existing: &mut Vec<String>, incoming: Vec<String>) {
    let mut seen: HashSet<String> = existing.iter().map(|item| item.to_lowercase()).collect();
    for item in incoming {
        if seen.insert(item.to_lowercase()) {
            existing.push(item);
        }
    }
    existing.truncate(12);
}

fn promoted_confidence(current: &str, evidence_count: u32) -> String {
    if evidence_count >= 2 || current == "medium" {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

fn skill_score(skill: &SpeechSkill, raw_text: &str) -> i32 {
    let raw = raw_text.to_lowercase();
    let trigger = skill.trigger.to_lowercase();
    let trigger_hit = raw.contains(&trigger);
    let context_hits = skill
        .context
        .iter()
        .filter(|term| {
            let term = term.to_lowercase();
            !is_generic_trigger(&term) && raw.contains(&term)
        })
        .count() as i32;

    if skill.kind == "english_phrase" && !trigger_hit && context_hits < 2 {
        return 0;
    }
    if is_generic_trigger(&trigger) && context_hits == 0 {
        return 0;
    }
    if !trigger_hit && context_hits == 0 {
        return 0;
    }

    let confidence_bonus = match skill.confidence.as_str() {
        "medium" => 2,
        "high" => 4,
        _ => 0,
    };
    let trigger_bonus = if trigger_hit { 6 } else { 0 };
    trigger_bonus + context_hits + confidence_bonus + skill.evidence_count.min(5) as i32
}

fn is_generic_trigger(trigger: &str) -> bool {
    matches!(trigger, "code" | "app" | "agent")
}

fn enabled() -> bool {
    std::env::var("OPENTYPELESS_LEARNING_CANDIDATES")
        .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}

fn jsonl_path() -> Option<PathBuf> {
    std::env::var_os("OPENTYPELESS_LEARNING_CANDIDATES_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".openless").join(DEFAULT_LEARNING_FILE))
        })
}

fn skill_path() -> Option<PathBuf> {
    std::env::var_os("OPENTYPELESS_SPEECH_SKILLS_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".openless").join(DEFAULT_SKILL_FILE))
        })
}

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn extracts_chinese_replacement_candidate() {
        let correction = extract_correction("这是一次融合测试。", "这是一次整合测试。").unwrap();

        assert_eq!(correction.kind, "replacement");
        assert_eq!(correction.from, "融");
        assert_eq!(correction.to, "整");
        assert_eq!(correction.prefix_chars, 4);
        assert_eq!(correction.suffix_chars, 4);
        assert_eq!(correction.confidence, "medium");
    }

    #[test]
    fn skips_unchanged_text() {
        assert!(extract_correction("OpenClaw", "OpenClaw").is_none());
    }

    #[test]
    fn extracts_change_against_full_field_baseline() {
        let correction = extract_correction(
            "这里是一个村。😔这里是一个测试。",
            "这里是一个村。😔这里是一个完整的测试。",
        )
        .unwrap();

        assert_eq!(correction.kind, "insertion");
        assert_eq!(correction.from, "");
        assert_eq!(correction.to, "完整的");
        assert_eq!(correction.confidence, "medium");
    }

    #[test]
    fn extracts_speech_skill_pairs_from_mixed_edit() {
        let pairs = extract_skill_pairs(
            "你想对表，不如说 col code 或 code",
            "我想对标说 claude code 或 codex",
        );

        assert!(pairs.contains(&(
            "col code".to_string(),
            "claude code".to_string(),
            "english_term"
        )));
        assert!(pairs.contains(&("code".to_string(), "codex".to_string(), "english_term")));
        assert!(pairs.contains(&("对表".to_string(), "对标".to_string(), "chinese_phrase")));
    }

    #[test]
    fn extracts_repeated_ambiguous_terms_as_phrase_skill() {
        let pairs = extract_skill_pairs("对表说 cold 或者 cold。😔", "对标说 claude code 或 codex");

        assert!(pairs.contains(&(
            "cold 或者 cold".to_string(),
            "claude code 或 codex".to_string(),
            "english_phrase"
        )));
        assert!(!pairs
            .iter()
            .any(|(trigger, correction, kind)| trigger == "cold"
                && correction == "codex"
                && *kind == "english_term"));
        assert!(!pairs
            .iter()
            .any(|(trigger, correction, kind)| trigger == "cold"
                && correction == "claude code"
                && *kind == "english_term"));
        assert!(pairs.contains(&("对表".to_string(), "对标".to_string(), "chinese_phrase")));
    }

    #[test]
    fn records_candidate_and_speech_skill_to_configured_files() {
        let _guard = ENV_LOCK.lock().unwrap();
        let path = std::env::temp_dir().join(format!(
            "opentypeless-learning-candidates-{}-{}.jsonl",
            std::process::id(),
            "payload"
        ));
        let skill_path = std::env::temp_dir().join(format!(
            "opentypeless-speech-skills-{}-{}.json",
            std::process::id(),
            "payload"
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&skill_path);
        std::env::set_var("OPENTYPELESS_LEARNING_CANDIDATES_PATH", &path);
        std::env::set_var("OPENTYPELESS_SPEECH_SKILLS_PATH", &skill_path);
        std::env::remove_var("OPENTYPELESS_LEARNING_CANDIDATES");

        let session = EditMonitorSession {
            target_pid: Some(42),
            original_text: "我想对表说 col code。".into(),
            inserted_text: "我想对表说 col code。".into(),
        };
        record_final_candidate(
            &session,
            42,
            "我想对表说 col code。",
            "我想对标说 Claude Code。",
        );

        let contents = std::fs::read_to_string(&path).unwrap();
        let value: Value = serde_json::from_str(contents.trim()).unwrap();
        assert_eq!(value["event"], "correction_candidate");
        assert_eq!(value["source"], "edit_monitor_final");
        assert_eq!(value["status"], "candidate");
        assert_eq!(value["targetPid"], 42);
        assert_eq!(value["originalText"], "我想对表说 col code。");
        assert_eq!(value["insertedText"], "我想对表说 col code。");
        assert_eq!(value["initialFieldValue"], "我想对表说 col code。");
        assert_eq!(value["finalText"], "我想对标说 Claude Code。");
        assert_eq!(value["correction"]["kind"], "replacement");

        let skills: Vec<SpeechSkill> =
            serde_json::from_str(&std::fs::read_to_string(&skill_path).unwrap()).unwrap();
        assert!(skills
            .iter()
            .any(|skill| skill.trigger == "col code" && skill.correction == "Claude Code"));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&skill_path);
        std::env::remove_var("OPENTYPELESS_LEARNING_CANDIDATES_PATH");
        std::env::remove_var("OPENTYPELESS_SPEECH_SKILLS_PATH");
    }

    #[test]
    fn retrieves_contextual_speech_skills_without_generic_code_false_positive() {
        let _guard = ENV_LOCK.lock().unwrap();
        let path = std::env::temp_dir().join(format!(
            "opentypeless-learning-candidates-{}-{}.jsonl",
            std::process::id(),
            "retrieve"
        ));
        let skill_path = std::env::temp_dir().join(format!(
            "opentypeless-speech-skills-{}-{}.json",
            std::process::id(),
            "retrieve"
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&skill_path);
        std::env::set_var("OPENTYPELESS_LEARNING_CANDIDATES_PATH", &path);
        std::env::set_var("OPENTYPELESS_SPEECH_SKILLS_PATH", &skill_path);
        std::env::remove_var("OPENTYPELESS_LEARNING_CANDIDATES");

        let session = EditMonitorSession {
            target_pid: Some(42),
            original_text: "你为想对表，不如说col code或 code。".into(),
            inserted_text: "你想对表，不如说 col code 或 code。".into(),
        };
        record_final_candidate(
            &session,
            42,
            "你想对表，不如说 col code 或 code。",
            "我想对标说 Claude Code 或 Codex。",
        );

        let skill_block = speech_skill_prompt_block("我想对表一下 col code 和 code").unwrap();
        assert!(skill_block.contains("Claude Code"));
        assert!(skill_block.contains("Codex"));
        assert!(skill_block.contains("对标"));

        let ordinary =
            speech_skill_prompt_block("今天我要写一段 code，顺便看代码风格").unwrap_or_default();
        assert!(!ordinary.contains("Codex"));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&skill_path);
        std::env::remove_var("OPENTYPELESS_LEARNING_CANDIDATES_PATH");
        std::env::remove_var("OPENTYPELESS_SPEECH_SKILLS_PATH");
    }

    #[test]
    fn retrieves_phrase_skill_only_with_enough_context() {
        let _guard = ENV_LOCK.lock().unwrap();
        let path = std::env::temp_dir().join(format!(
            "opentypeless-learning-candidates-{}-{}.jsonl",
            std::process::id(),
            "phrase"
        ));
        let skill_path = std::env::temp_dir().join(format!(
            "opentypeless-speech-skills-{}-{}.json",
            std::process::id(),
            "phrase"
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&skill_path);
        std::env::set_var("OPENTYPELESS_LEARNING_CANDIDATES_PATH", &path);
        std::env::set_var("OPENTYPELESS_SPEECH_SKILLS_PATH", &skill_path);
        std::env::remove_var("OPENTYPELESS_LEARNING_CANDIDATES");

        let session = EditMonitorSession {
            target_pid: Some(42),
            original_text: "我想对表说co cold或者 cold。".into(),
            inserted_text: "我想对表说 cold 或者 cold。".into(),
        };
        record_final_candidate(
            &session,
            42,
            "我想对表说 cold 或者 cold。😔",
            "我想对标说 Claude Code 或 Codex",
        );

        let skills: Vec<SpeechSkill> =
            serde_json::from_str(&std::fs::read_to_string(&skill_path).unwrap()).unwrap();
        assert!(skills.iter().any(|skill| {
            skill.trigger == "cold 或者 cold"
                && skill.correction == "Claude Code 或 Codex"
                && skill.kind == "english_phrase"
        }));
        assert!(!skills.iter().any(|skill| {
            skill.trigger == "cold" && matches!(skill.correction.as_str(), "Claude Code" | "Codex")
        }));

        let contextual = speech_skill_prompt_block("我想对表一下 cold cold 和 cold").unwrap();
        assert!(contextual.contains("Claude Code 或 Codex"));

        let ordinary = speech_skill_prompt_block("今天外面很 cold").unwrap_or_default();
        assert!(!ordinary.contains("Claude Code 或 Codex"));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&skill_path);
        std::env::remove_var("OPENTYPELESS_LEARNING_CANDIDATES_PATH");
        std::env::remove_var("OPENTYPELESS_SPEECH_SKILLS_PATH");
    }

    #[test]
    fn filters_ambiguous_stored_term_skills() {
        let filtered = filter_ambiguous_stored_skills(vec![
            test_skill("cold", "Codex", "english_term"),
            test_skill("cold", "Claude Code", "english_term"),
            test_skill("cold 或者 cold", "Claude Code 或 Codex", "english_phrase"),
            test_skill("对表", "对标", "chinese_phrase"),
        ]);

        assert!(!filtered
            .iter()
            .any(|skill| skill.trigger == "cold" && skill.kind == "english_term"));
        assert!(filtered
            .iter()
            .any(|skill| skill.trigger == "cold 或者 cold" && skill.kind == "english_phrase"));
        assert!(filtered
            .iter()
            .any(|skill| skill.trigger == "对表" && skill.kind == "chinese_phrase"));
    }

    fn test_skill(trigger: &str, correction: &str, kind: &str) -> SpeechSkill {
        SpeechSkill {
            id: format!("{trigger}-{correction}"),
            trigger: trigger.to_string(),
            correction: correction.to_string(),
            context: vec!["cold".to_string(), "对表".to_string()],
            kind: kind.to_string(),
            confidence: "low".to_string(),
            evidence_count: 1,
            last_evidence: SkillEvidence {
                original_text: String::new(),
                inserted_text: String::new(),
                initial_field_value: String::new(),
                final_text: String::new(),
                from: String::new(),
                to: String::new(),
                target_pid: 0,
            },
            created_at: "2026-05-10T00:00:00Z".to_string(),
            updated_at: "2026-05-10T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn respects_disable_flag() {
        let _guard = ENV_LOCK.lock().unwrap();
        let path = std::env::temp_dir().join(format!(
            "opentypeless-learning-candidates-{}-{}.jsonl",
            std::process::id(),
            "disabled"
        ));
        let skill_path = std::env::temp_dir().join(format!(
            "opentypeless-speech-skills-{}-{}.json",
            std::process::id(),
            "disabled"
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&skill_path);
        std::env::set_var("OPENTYPELESS_LEARNING_CANDIDATES_PATH", &path);
        std::env::set_var("OPENTYPELESS_SPEECH_SKILLS_PATH", &skill_path);
        std::env::set_var("OPENTYPELESS_LEARNING_CANDIDATES", "0");

        let session = EditMonitorSession {
            target_pid: Some(42),
            original_text: "raw".into(),
            inserted_text: "polished".into(),
        };
        record_final_candidate(&session, 42, "polished", "edited");

        assert!(!path.exists());
        assert!(!skill_path.exists());

        std::env::remove_var("OPENTYPELESS_LEARNING_CANDIDATES");
        std::env::remove_var("OPENTYPELESS_LEARNING_CANDIDATES_PATH");
        std::env::remove_var("OPENTYPELESS_SPEECH_SKILLS_PATH");
    }
}
