//! Picker search (SPEC 10.2).
//!
//! [`SearchIndex`] keeps every display row's preview in memory for fuzzy matching
//! with `nucleo-matcher`; queries of 3+ characters also run FTS5 over full text and
//! append those hits after the fuzzy ones.

use std::collections::{HashMap, HashSet};

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32String};

use crate::error::Result;
use crate::hlc::wall_ms;
use crate::model::{ClipKind, DeviceId, DeviceInfo, Hash32};
use crate::store::{DisplayRow, ReadView};

/// A parsed query: filter tokens plus the remaining free text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    /// `is:pinned`
    pub pinned: bool,
    /// `is:image`, `is:rich`, `is:text` (any of them).
    pub kinds: Vec<ClipKind>,
    /// `from:<device name>` (any of them). Quote names with spaces: `from:"Work Mac"`.
    pub from: Vec<String>,
    pub today: bool,
    pub yesterday: bool,
    /// Everything else, joined with single spaces.
    pub text: String,
}

/// Splits on whitespace, keeping `"quoted strings"` together (quotes removed).
fn tokens(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in query.chars() {
        match ch {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Parses `is:pinned is:image is:rich is:text from:<device> today yesterday`; other
/// words are free text. Tokens are case-insensitive.
pub fn parse_query(query: &str) -> ParsedQuery {
    let mut q = ParsedQuery::default();
    let mut text = Vec::new();
    for token in tokens(query) {
        let lower = token.to_lowercase();
        match lower.as_str() {
            "is:pinned" => q.pinned = true,
            "is:image" => q.kinds.push(ClipKind::Image),
            "is:rich" => q.kinds.push(ClipKind::Rich),
            "is:text" => q.kinds.push(ClipKind::Text),
            "today" => q.today = true,
            "yesterday" => q.yesterday = true,
            _ => match lower.strip_prefix("from:") {
                Some(name) if !name.is_empty() => q.from.push(name.to_owned()),
                _ => text.push(token),
            },
        }
    }
    q.text = text.join(" ");
    q
}

/// Local day boundaries, which the app computes in the user's time zone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchContext {
    /// Unix ms of local midnight today.
    pub today_start_ms: u64,
    /// Unix ms of local midnight yesterday.
    pub yesterday_start_ms: u64,
}

/// Why a row is in the results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitSource {
    /// No free text: filters only, list order.
    Filter,
    /// Fuzzy match on the preview or label.
    Fuzzy,
    /// FTS5 match on the full text.
    FullText,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub row: DisplayRow,
    pub source: HitSource,
    /// Fuzzy score (higher is better); 0 for other sources.
    pub score: u32,
}

struct Entry {
    row: DisplayRow,
    preview: Utf32String,
    label: Option<Utf32String>,
}

impl Entry {
    fn new(row: DisplayRow) -> Self {
        Entry {
            preview: Utf32String::from(row.preview.as_str()),
            label: row.label.as_deref().map(Utf32String::from),
            row,
        }
    }
}

/// In-memory index of display rows, kept warm by the app.
pub struct SearchIndex {
    /// In list order: `sort_hlc DESC, clip_id DESC`.
    entries: Vec<Entry>,
    by_group: HashMap<Hash32, usize>,
    devices: Vec<DeviceInfo>,
    matcher: Matcher,
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Free text this long or longer also searches full text (SPEC 10.2).
pub const FTS_MIN_CHARS: usize = 3;

/// Greedy-pass candidates re-scored with the optimal matcher, at least.
const RESCORE_MIN: usize = 100;

fn list_order(a: &DisplayRow, b: &DisplayRow) -> std::cmp::Ordering {
    (b.sort_hlc, &b.clip_id).cmp(&(a.sort_hlc, &a.clip_id))
}

impl SearchIndex {
    pub fn new() -> Self {
        SearchIndex {
            entries: Vec::new(),
            by_group: HashMap::new(),
            devices: Vec::new(),
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    /// Builds the index from every display row and device.
    pub fn load(view: &ReadView<'_>) -> Result<Self> {
        let mut index = Self::new();
        index.reload(view)?;
        Ok(index)
    }

    pub fn reload(&mut self, view: &ReadView<'_>) -> Result<()> {
        let rows = view.all_rows()?;
        self.devices = view.devices()?;
        self.set_rows(rows);
        Ok(())
    }

    /// Replaces all rows (any order).
    pub fn set_rows(&mut self, mut rows: Vec<DisplayRow>) {
        rows.sort_by(list_order);
        self.entries = rows.into_iter().map(Entry::new).collect();
        self.reindex();
    }

    pub fn set_devices(&mut self, devices: Vec<DeviceInfo>) {
        self.devices = devices;
    }

    /// Re-reads the given display groups (from [`crate::Store::take_dirty_groups`]).
    pub fn refresh_groups(&mut self, view: &ReadView<'_>, groups: &[Hash32]) -> Result<()> {
        if groups.is_empty() {
            return Ok(());
        }
        if groups.len() > self.entries.len() / 4 + 16 {
            return self.reload(view);
        }
        let wanted: HashSet<&Hash32> = groups.iter().collect();
        self.entries.retain(|e| !wanted.contains(&e.row.group_key));
        for key in groups {
            if let Some(row) = view.row_for_group(key)? {
                let pos = self
                    .entries
                    .binary_search_by(|e| list_order(&e.row, &row))
                    .unwrap_or_else(|p| p);
                self.entries.insert(pos, Entry::new(row));
            }
        }
        self.reindex();
        Ok(())
    }

    fn reindex(&mut self) {
        self.by_group = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.row.group_key, i))
            .collect();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Rows in list order, for the picker's first frame.
    pub fn rows(&self) -> impl Iterator<Item = &DisplayRow> {
        self.entries.iter().map(|e| &e.row)
    }

    fn device_ids(&self, names: &[String]) -> HashSet<DeviceId> {
        self.devices
            .iter()
            .filter(|d| {
                let name = d.display_name.to_lowercase();
                let host = d.ts_hostname.as_deref().unwrap_or("").to_lowercase();
                names.iter().any(|n| {
                    (!name.is_empty() && name.starts_with(n.as_str()))
                        || (!host.is_empty() && host.starts_with(n.as_str()))
                })
            })
            .map(|d| d.device_id.clone())
            .collect()
    }

    /// Runs a query. `view` enables full-text search; pass `None` to search previews
    /// only.
    pub fn search(
        &mut self,
        view: Option<&ReadView<'_>>,
        query: &str,
        ctx: &SearchContext,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let q = parse_query(query);
        let from = (!q.from.is_empty()).then(|| self.device_ids(&q.from));
        let today_start = ctx.today_start_ms;
        let yesterday_start = ctx.yesterday_start_ms;
        let passes = |row: &DisplayRow| -> bool {
            if q.pinned && !row.pinned {
                return false;
            }
            if !q.kinds.is_empty() && !q.kinds.contains(&row.kind) {
                return false;
            }
            if let Some(ids) = &from
                && !ids.contains(&row.origin)
            {
                return false;
            }
            if q.today || q.yesterday {
                let t = wall_ms(row.sort_hlc);
                let in_today = t >= today_start;
                let in_yesterday = t >= yesterday_start && t < today_start;
                if !((q.today && in_today) || (q.yesterday && in_yesterday)) {
                    return false;
                }
            }
            true
        };

        let text = q.text.trim();
        if text.is_empty() {
            return Ok(self
                .entries
                .iter()
                .filter(|e| passes(&e.row))
                .take(limit)
                .map(|e| SearchHit {
                    row: e.row.clone(),
                    source: HitSource::Filter,
                    score: 0,
                })
                .collect());
        }

        // Two passes keep this inside the 16 ms budget (SPEC 15): a linear greedy match
        // over every preview finds what matches at all, then the optimal (Smith-Waterman)
        // score ranks the best few hundred candidates.
        let atoms: Vec<(Atom, bool)> = text
            .split_whitespace()
            .map(|word| {
                let atom = Atom::new(
                    word,
                    CaseMatching::Smart,
                    Normalization::Smart,
                    AtomKind::Fuzzy,
                    false,
                );
                let ignore_case = !word.is_ascii() || !word.bytes().any(|b| b.is_ascii_uppercase());
                (atom, ignore_case)
            })
            .collect();
        let matcher = &mut self.matcher;
        let greedy = |matcher: &mut Matcher, hay: &Utf32String| -> Option<u32> {
            let mut total = 0u32;
            for (atom, ignore_case) in &atoms {
                matcher.config.ignore_case = *ignore_case;
                matcher.config.normalize = true;
                total += u32::from(matcher.fuzzy_match_greedy(hay.slice(..), atom.needle_text())?);
            }
            Some(total)
        };
        let mut candidates: Vec<(u32, usize)> = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            if !passes(&e.row) {
                continue;
            }
            let preview = greedy(matcher, &e.preview);
            let label = e.label.as_ref().and_then(|l| greedy(matcher, l));
            if let Some(score) = preview.max(label) {
                candidates.push((score, i));
            }
        }
        let keep = (limit * 2).max(RESCORE_MIN);
        if candidates.len() > keep {
            candidates.select_nth_unstable_by(keep, |a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            candidates.truncate(keep);
        }
        let optimal = |matcher: &mut Matcher, hay: &Utf32String| -> Option<u32> {
            let mut total = 0u32;
            for (atom, _) in &atoms {
                total += u32::from(atom.score(hay.slice(..), matcher)?);
            }
            Some(total)
        };
        let mut scored: Vec<(u32, usize)> = candidates
            .into_iter()
            .filter_map(|(_, i)| {
                let e = &self.entries[i];
                let preview = optimal(matcher, &e.preview);
                let label = e.label.as_ref().and_then(|l| optimal(matcher, l));
                preview.max(label).map(|score| (score, i))
            })
            .collect();
        // Best score first; ties keep list order.
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.truncate(limit);

        let mut seen: HashSet<usize> = scored.iter().map(|s| s.1).collect();
        let mut hits: Vec<SearchHit> = scored
            .iter()
            .map(|&(score, i)| SearchHit {
                row: self.entries[i].row.clone(),
                source: HitSource::Fuzzy,
                score,
            })
            .collect();

        if let Some(view) = view
            && text.chars().count() >= FTS_MIN_CHARS
            && hits.len() < limit
        {
            for (_, group) in view.fts_search(text, limit * 2)? {
                if hits.len() >= limit {
                    break;
                }
                let Some(&i) = self.by_group.get(&group) else {
                    continue;
                };
                if !passes(&self.entries[i].row) || !seen.insert(i) {
                    continue;
                }
                hits.push(SearchHit {
                    row: self.entries[i].row.clone(),
                    source: HitSource::FullText,
                    score: 0,
                });
            }
        }
        Ok(hits)
    }
}

#[cfg(test)]
mod tests;
