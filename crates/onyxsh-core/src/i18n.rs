//! Internationalization (i18n) catalog engine (Port of `src/onyxsh/utils/translation_utils.py`).
//!
//! Loads compiled GNU gettext `.mo` catalogs natively in pure Rust across all 28 supported
//! languages (`bg`, `cs`, `da`, `de`, `el`, `en`, `es`, `et`, `fi`, `fr`, `he`, `hr`, `hu`, `is`,
//! `it`, `ja`, `ko`, `nl`, `no`, `pl`, `pt`, `pt_BR`, `ro`, `ru`, `sk`, `sv`, `tr`, `uk`, `zh`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

const GETTEXT_DOMAIN: &str = "onyxsh";
const MO_MAGIC_LE: u32 = 0x9504_12de;
const MO_MAGIC_BE: u32 = 0xde12_0495;

#[derive(Debug, Clone, Default)]
struct MoCatalog {
    singular: HashMap<String, String>,
    plural: HashMap<String, Vec<String>>,
    plural_expr: Option<String>,
}

#[derive(Debug)]
struct I18nState {
    active_locale: String,
    locale_dir: Option<PathBuf>,
    catalog: MoCatalog,
}

static I18N_STATE: OnceLock<RwLock<I18nState>> = OnceLock::new();

fn state_lock() -> &'static RwLock<I18nState> {
    I18N_STATE.get_or_init(|| {
        let mut st = I18nState {
            active_locale: String::from("C"),
            locale_dir: None,
            catalog: MoCatalog::default(),
        };
        load_system_locale_into(&mut st);
        RwLock::new(st)
    })
}

fn read_u32(bytes: &[u8], offset: usize, little_endian: bool) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    let arr: [u8; 4] = slice.try_into().ok()?;
    Some(if little_endian {
        u32::from_le_bytes(arr)
    } else {
        u32::from_be_bytes(arr)
    })
}

/// Parses a binary GNU gettext `.mo` catalog in pure Rust (little-endian or big-endian).
fn parse_mo_bytes(bytes: &[u8]) -> Result<MoCatalog, String> {
    if bytes.len() < 28 {
        return Err("Invalid .mo file: header shorter than 28 bytes".to_string());
    }

    let raw_magic = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| "bad magic slice")?);
    let little_endian = if raw_magic == MO_MAGIC_LE {
        true
    } else if raw_magic == MO_MAGIC_BE {
        false
    } else {
        return Err(format!("Invalid .mo magic header: 0x{raw_magic:08x}"));
    };

    let revision = read_u32(bytes, 4, little_endian).ok_or("Missing revision")?;
    let major = (revision >> 16) & 0xffff;
    if major > 1 {
        return Err(format!("Unsupported .mo major revision: {major}"));
    }

    let num_strings = read_u32(bytes, 8, little_endian).ok_or("Missing num_strings")? as usize;
    let orig_table_off =
        read_u32(bytes, 12, little_endian).ok_or("Missing orig_table_off")? as usize;
    let trans_table_off =
        read_u32(bytes, 16, little_endian).ok_or("Missing trans_table_off")? as usize;

    let table_bytes = num_strings
        .checked_mul(8)
        .ok_or("Integer overflow in .mo table size")?;
    if orig_table_off
        .checked_add(table_bytes)
        .is_none_or(|end| end > bytes.len())
        || trans_table_off
            .checked_add(table_bytes)
            .is_none_or(|end| end > bytes.len())
    {
        return Err("Corrupt .mo file: string offset table out of bounds".to_string());
    }

    let mut cat = MoCatalog {
        singular: HashMap::with_capacity(num_strings),
        plural: HashMap::new(),
        plural_expr: None,
    };

    for i in 0..num_strings {
        let orig_entry_off = orig_table_off + i * 8;
        let trans_entry_off = trans_table_off + i * 8;

        let orig_len = read_u32(bytes, orig_entry_off, little_endian).unwrap_or(0) as usize;
        let orig_off = read_u32(bytes, orig_entry_off + 4, little_endian).unwrap_or(0) as usize;
        let trans_len = read_u32(bytes, trans_entry_off, little_endian).unwrap_or(0) as usize;
        let trans_off = read_u32(bytes, trans_entry_off + 4, little_endian).unwrap_or(0) as usize;

        let orig_end = orig_off
            .checked_add(orig_len)
            .ok_or("Corrupt .mo orig string bounds")?;
        let trans_end = trans_off
            .checked_add(trans_len)
            .ok_or("Corrupt .mo trans string bounds")?;

        if orig_end > bytes.len() || trans_end > bytes.len() {
            return Err("Corrupt .mo file: string slice out of bounds".to_string());
        }

        let orig_raw = std::str::from_utf8(&bytes[orig_off..orig_end])
            .map_err(|e| format!("Invalid UTF-8 in .mo msgid: {e}"))?;
        let trans_raw = std::str::from_utf8(&bytes[trans_off..trans_end])
            .map_err(|e| format!("Invalid UTF-8 in .mo msgstr: {e}"))?;

        if orig_raw.is_empty() {
            for line in trans_raw.lines() {
                if let Some(rest) = line.strip_prefix("Plural-Forms:") {
                    if let Some((_, expr)) = rest.split_once("plural=") {
                        let clean = expr.trim().trim_end_matches(';').trim().to_string();
                        if !clean.is_empty() {
                            cat.plural_expr = Some(clean);
                        }
                    }
                }
            }
            continue;
        }

        if trans_raw.is_empty() {
            continue;
        }

        if let Some((singular_id, _plural_id)) = orig_raw.split_once('\0') {
            let forms: Vec<String> = trans_raw.split('\0').map(str::to_string).collect();
            if let Some(first) = forms.first() {
                if !first.is_empty() {
                    cat.singular.insert(singular_id.to_string(), first.clone());
                }
            }
            cat.plural.insert(singular_id.to_string(), forms);
        } else {
            cat.singular
                .insert(orig_raw.to_string(), trans_raw.to_string());
        }
    }

    Ok(cat)
}

/// Evaluates the subset of C plural expressions used in GNU `.po`/`.mo` headers.
fn evaluate_plural_index(expr: Option<&str>, n: u64, num_forms: usize) -> usize {
    let idx = match expr {
        None => usize::from(n != 1),
        Some(raw) => eval_ternary(raw.trim(), n as i64).unwrap_or(i64::from(n != 1)).max(0) as usize,
    };
    if num_forms == 0 {
        0
    } else {
        idx.min(num_forms - 1)
    }
}

fn strip_outer_parens(mut s: &str) -> &str {
    loop {
        s = s.trim();
        if s.starts_with('(') && s.ends_with(')') {
            let mut depth = 0i32;
            let mut wraps_whole = true;
            for (i, ch) in s.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 && i + ch.len_utf8() < s.len() {
                            wraps_whole = false;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if wraps_whole && depth == 0 {
                s = &s[1..s.len() - 1];
                continue;
            }
        }
        return s;
    }
}

fn find_top_level_op<'a>(s: &'a str, op: &str) -> Option<usize> {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let op_bytes = op.as_bytes();
    let mut i = 0;
    while i + op_bytes.len() <= bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0 && &bytes[i..i + op_bytes.len()] == op_bytes {
            // Avoid matching '=' inside '==', '!=', '<=', '>=' or '%'/'?' confusion
            if op == "<" && bytes.get(i + 1) == Some(&b'=') {
                i += 2;
                continue;
            }
            if op == ">" && bytes.get(i + 1) == Some(&b'=') {
                i += 2;
                continue;
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

fn eval_ternary(s: &str, n: i64) -> Option<i64> {
    let s = strip_outer_parens(s);
    if let Some(q_idx) = find_top_level_op(s, "?") {
        // Find matching ':' at depth 0 accounting for nested ternaries
        let mut depth = 0i32;
        let mut ternary_depth = 0i32;
        let rest = &s[q_idx + 1..];
        let mut colon_rel = None;
        for (idx, ch) in rest.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                '?' if depth == 0 => ternary_depth += 1,
                ':' if depth == 0 => {
                    if ternary_depth == 0 {
                        colon_rel = Some(idx);
                        break;
                    }
                    ternary_depth -= 1;
                }
                _ => {}
            }
        }
        if let Some(c_rel) = colon_rel {
            let cond = eval_or(&s[..q_idx], n)?;
            let branch_t = &rest[..c_rel];
            let branch_f = &rest[c_rel + 1..];
            return if cond != 0 {
                eval_ternary(branch_t, n)
            } else {
                eval_ternary(branch_f, n)
            };
        }
    }
    eval_or(s, n)
}

fn eval_or(s: &str, n: i64) -> Option<i64> {
    let s = strip_outer_parens(s);
    if let Some(pos) = find_top_level_op(s, "||") {
        let l = eval_or(&s[..pos], n)?;
        let r = eval_and(&s[pos + 2..], n)?;
        return Some(i64::from(l != 0 || r != 0));
    }
    eval_and(s, n)
}

fn eval_and(s: &str, n: i64) -> Option<i64> {
    let s = strip_outer_parens(s);
    if let Some(pos) = find_top_level_op(s, "&&") {
        let l = eval_and(&s[..pos], n)?;
        let r = eval_cmp(&s[pos + 2..], n)?;
        return Some(i64::from(l != 0 && r != 0));
    }
    eval_cmp(s, n)
}

fn eval_cmp(s: &str, n: i64) -> Option<i64> {
    let s = strip_outer_parens(s);
    for op in ["==", "!=", "<=", ">=", "<", ">"] {
        if let Some(pos) = find_top_level_op(s, op) {
            let l = eval_mod(&s[..pos], n)?;
            let r = eval_mod(&s[pos + op.len()..], n)?;
            let res = match op {
                "==" => l == r,
                "!=" => l != r,
                "<=" => l <= r,
                ">=" => l >= r,
                "<" => l < r,
                ">" => l > r,
                _ => false,
            };
            return Some(i64::from(res));
        }
    }
    eval_mod(s, n)
}

fn eval_mod(s: &str, n: i64) -> Option<i64> {
    let s = strip_outer_parens(s);
    if let Some(pos) = find_top_level_op(s, "%") {
        let l = eval_mod(&s[..pos], n)?;
        let r = eval_atom(&s[pos + 1..], n)?;
        return if r == 0 { None } else { Some(l % r) };
    }
    eval_atom(s, n)
}

fn eval_atom(s: &str, n: i64) -> Option<i64> {
    let s = strip_outer_parens(s);
    if s == "n" {
        return Some(n);
    }
    if let Some(inner) = s.strip_prefix('!') {
        let v = eval_atom(inner, n)?;
        return Some(i64::from(v == 0));
    }
    s.parse::<i64>().ok()
}

/// Returns candidate locale directories in priority order mirroring `translation_utils.py`.
pub fn candidate_locale_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(env_dir) = std::env::var("ONYXSH_LOCALE_DIR") {
        if !env_dir.trim().is_empty() {
            dirs.push(PathBuf::from(env_dir));
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            dirs.push(exe_dir.join("locale"));
            dirs.push(exe_dir.join("../share/locale"));
            dirs.push(exe_dir.join("../lib/onyxsh/locale"));
            dirs.push(exe_dir.join("../../src/onyxsh/locale"));
            dirs.push(exe_dir.join("../../../src/onyxsh/locale"));
        }
    }

    if let Ok(appdir) = std::env::var("APPDIR") {
        if !appdir.trim().is_empty() {
            dirs.push(PathBuf::from(&appdir).join("usr/share/locale"));
            dirs.push(PathBuf::from(appdir).join("usr/lib/onyxsh/locale"));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("src/onyxsh/locale"));
    }

    // Compile-time workspace path so dev binaries/tests launched outside repo root still find catalogs.
    let manifest_locale = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src/onyxsh/locale");
    dirs.push(manifest_locale);

    dirs.push(PathBuf::from("/app/lib/onyxsh/locale"));
    dirs.push(PathBuf::from("/app/share/locale"));
    dirs.push(PathBuf::from("/usr/lib/onyxsh/locale"));
    dirs.push(PathBuf::from("/usr/share/locale"));
    dirs.push(PathBuf::from("/usr/local/share/locale"));
    dirs.push(PathBuf::from("/opt/onyxsh/locale"));

    dirs
}

/// Expands a POSIX locale identifier (`"pt_BR.UTF-8"` or `"pt_BR:pt:en"`) into lookup candidates.
pub fn expand_locale_candidates(raw_locale: &str) -> Vec<String> {
    let mut out = Vec::new();
    for segment in raw_locale.split(':') {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        let no_encoding = trimmed.split('.').next().unwrap_or(trimmed);
        let base = no_encoding.split('@').next().unwrap_or(no_encoding).trim();
        if base.is_empty() || base.eq_ignore_ascii_case("C") || base.eq_ignore_ascii_case("POSIX") {
            continue;
        }
        if !out.iter().any(|existing| existing == base) {
            out.push(base.to_string());
        }
        if let Some((lang, _region)) = base.split_once('_') {
            if !lang.is_empty() && !out.iter().any(|existing| existing == lang) {
                out.push(lang.to_string());
            }
        }
        if let Some((lang, _region)) = base.split_once('-') {
            if !lang.is_empty() && !out.iter().any(|existing| existing == lang) {
                out.push(lang.to_string());
            }
        }
    }
    out
}

/// Determines the preferred locale string from POSIX environment variables.
fn detect_env_locale() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            let trimmed = val.trim();
            if trimmed.eq_ignore_ascii_case("C")
                || trimmed.eq_ignore_ascii_case("POSIX")
                || trimmed.starts_with("C.")
            {
                return "C".to_string();
            }
        }
    }

    if let Ok(language) = std::env::var("LANGUAGE") {
        let trimmed = language.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    "C".to_string()
}

fn find_mo_file_for_candidates(candidates: &[String]) -> Option<(String, PathBuf, PathBuf)> {
    let dirs = candidate_locale_dirs();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        for lang in candidates {
            let mo_path = dir
                .join(lang)
                .join("LC_MESSAGES")
                .join(format!("{GETTEXT_DOMAIN}.mo"));
            if mo_path.is_file() {
                return Some((lang.clone(), dir, mo_path));
            }
        }
    }
    None
}

fn load_catalog_for_locale_str(raw_locale: &str) -> (String, Option<PathBuf>, MoCatalog) {
    let candidates = expand_locale_candidates(raw_locale);
    if candidates.is_empty() {
        return ("C".to_string(), None, MoCatalog::default());
    }

    if let Some((matched_lang, dir, mo_path)) = find_mo_file_for_candidates(&candidates) {
        match fs::read(&mo_path).and_then(|b| parse_mo_bytes(&b).map_err(std::io::Error::other)) {
            Ok(catalog) => return (matched_lang, Some(dir), catalog),
            Err(err) => {
                tracing::warn!(path = %mo_path.display(), error = %err, "Failed to parse .mo catalog");
            }
        }
    }

    (candidates[0].clone(), None, MoCatalog::default())
}

fn load_system_locale_into(st: &mut I18nState) {
    let env_locale = detect_env_locale();
    let (active, dir, catalog) = load_catalog_for_locale_str(&env_locale);
    st.active_locale = active;
    st.locale_dir = dir;
    st.catalog = catalog;
}

/// Initializes internationalization from POSIX environment variables and returns the active locale code.
pub fn init_i18n() -> String {
    if let Ok(mut st) = state_lock().write() {
        load_system_locale_into(&mut st);
        tracing::info!(
            locale = %st.active_locale,
            entries = st.catalog.singular.len(),
            locale_dir = ?st.locale_dir,
            "Initialized OnyxSH i18n catalog"
        );
        return st.active_locale.clone();
    }
    "C".to_string()
}

/// Switches the active runtime locale cleanly (reloading the `.mo` catalog) and returns resolved code.
pub fn set_locale(locale: &str) -> String {
    let (active, dir, catalog) = load_catalog_for_locale_str(locale);
    if let Ok(mut st) = state_lock().write() {
        st.active_locale = active.clone();
        st.locale_dir = dir;
        st.catalog = catalog;
    }
    active
}

/// Returns the currently active locale identifier (e.g. `"pt_BR"`, `"pt"`, `"es"`, `"C"`).
pub fn active_locale() -> String {
    state_lock()
        .read()
        .map(|st| st.active_locale.clone())
        .unwrap_or_else(|_| "C".to_string())
}

/// Lists all available compiled `.mo` locale codes discovered in candidate locale directories.
pub fn supported_locales() -> Vec<String> {
    let mut langs = Vec::new();
    for dir in candidate_locale_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let mo_file = p.join("LC_MESSAGES").join(format!("{GETTEXT_DOMAIN}.mo"));
            if mo_file.is_file() {
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if !langs.iter().any(|existing| existing == name) {
                        langs.push(name.to_string());
                    }
                }
            }
        }
        if !langs.is_empty() {
            break;
        }
    }
    langs.sort();
    langs
}

/// Translates `msgid` using the active GNU `.mo` catalog, falling back to `msgid` when untranslated.
pub fn tr(msgid: &str) -> String {
    if msgid.is_empty() {
        return String::new();
    }
    if let Ok(guard) = state_lock().read() {
        if let Some(translated) = guard.catalog.singular.get(msgid) {
            return translated.clone();
        }
    }
    msgid.to_string()
}

/// Translates `msgid` and replaces `{key}` placeholders in order from `args`.
pub fn tr_fmt(msgid: &str, args: &[(&str, &str)]) -> String {
    let mut out = tr(msgid);
    for (k, v) in args {
        let needle = format!("{{{k}}}");
        out = out.replace(&needle, v);
    }
    out
}

/// Translates singular/plural forms using `.mo` `Plural-Forms` expression rules.
pub fn tr_n(msgid: &str, msgid_plural: &str, n: u64) -> String {
    if let Ok(guard) = state_lock().read() {
        if let Some(forms) = guard.catalog.plural.get(msgid) {
            let idx = evaluate_plural_index(
                guard.catalog.plural_expr.as_deref(),
                n,
                forms.len(),
            );
            if let Some(chosen) = forms.get(idx) {
                if !chosen.is_empty() {
                    return chosen.clone();
                }
            }
        }
        if n == 1 {
            if let Some(t) = guard.catalog.singular.get(msgid) {
                return t.clone();
            }
        } else if let Some(t) = guard.catalog.singular.get(msgid_plural) {
            return t.clone();
        }
    }
    if n == 1 {
        msgid.to_string()
    } else {
        msgid_plural.to_string()
    }
}

/// Translates `msgid` against an explicit `locale` without mutating global i18n state.
pub fn tr_for_locale(locale: &str, msgid: &str) -> String {
    if msgid.is_empty() {
        return String::new();
    }
    let (_, _, catalog) = load_catalog_for_locale_str(locale);
    catalog
        .singular
        .get(msgid)
        .cloned()
        .unwrap_or_else(|| msgid.to_string())
}

/// Ensures `locale` directory lookup works smoothly from `Path` overrides in tests.
pub fn parse_mo_file(path: &Path) -> Result<usize, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let cat = parse_mo_bytes(&bytes)?;
    Ok(cat.singular.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCALE_LOCK: Mutex<()> = Mutex::new(());

    fn build_synthetic_mo(entries: &[(&str, &str)], little_endian: bool) -> Vec<u8> {
        let mut sorted: Vec<(&str, &str)> = entries.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(b.0));

        let n = sorted.len();
        let header_size = 28usize;
        let orig_table_off = header_size;
        let trans_table_off = orig_table_off + n * 8;
        let data_off = trans_table_off + n * 8;

        let mut orig_meta = Vec::with_capacity(n);
        let mut trans_meta = Vec::with_capacity(n);
        let mut string_blob = Vec::new();

        for (k, _) in &sorted {
            let len = k.len();
            orig_meta.push((len, data_off + string_blob.len()));
            string_blob.extend_from_slice(k.as_bytes());
            string_blob.push(0);
        }
        for (_, v) in &sorted {
            let len = v.len();
            trans_meta.push((len, data_off + string_blob.len()));
            string_blob.extend_from_slice(v.as_bytes());
            string_blob.push(0);
        }

        let encode_u32 = |val: u32| -> [u8; 4] {
            if little_endian {
                val.to_le_bytes()
            } else {
                val.to_be_bytes()
            }
        };

        let mut out = Vec::new();
        out.extend_from_slice(&encode_u32(MO_MAGIC_LE));
        out.extend_from_slice(&encode_u32(0));
        out.extend_from_slice(&encode_u32(n as u32));
        out.extend_from_slice(&encode_u32(orig_table_off as u32));
        out.extend_from_slice(&encode_u32(trans_table_off as u32));
        out.extend_from_slice(&encode_u32(0));
        out.extend_from_slice(&encode_u32(0));

        for (len, off) in orig_meta {
            out.extend_from_slice(&encode_u32(len as u32));
            out.extend_from_slice(&encode_u32(off as u32));
        }
        for (len, off) in trans_meta {
            out.extend_from_slice(&encode_u32(len as u32));
            out.extend_from_slice(&encode_u32(off as u32));
        }
        out.extend_from_slice(&string_blob);
        out
    }

    #[test]
    fn test_parse_synthetic_mo_le_and_be_including_plurals() {
        let entries = [
            (
                "",
                "Content-Type: text/plain; charset=UTF-8\nPlural-Forms: nplurals=2; plural=(n != 1);\n",
            ),
            ("Preferences", "Preferências"),
            ("1 file\0{n} files", "1 arquivo\0{n} arquivos"),
        ];

        for le in [true, false] {
            let raw = build_synthetic_mo(&entries, le);
            let cat = parse_mo_bytes(&raw).expect("synthetic .mo should parse cleanly");
            assert_eq!(cat.singular.get("Preferences").map(String::as_str), Some("Preferências"));
            assert_eq!(cat.plural_expr.as_deref(), Some("(n != 1)"));
            assert_eq!(
                cat.plural.get("1 file").cloned(),
                Some(vec!["1 arquivo".to_string(), "{n} arquivos".to_string()])
            );
            assert_eq!(evaluate_plural_index(cat.plural_expr.as_deref(), 1, 2), 0);
            assert_eq!(evaluate_plural_index(cat.plural_expr.as_deref(), 5, 2), 1);
        }
    }

    #[test]
    fn test_expand_locale_candidates() {
        assert_eq!(expand_locale_candidates("pt_BR.UTF-8"), vec!["pt_BR", "pt"]);
        assert_eq!(
            expand_locale_candidates("es_ES@euro:es:en_US.UTF-8"),
            vec!["es_ES", "es", "en_US", "en"]
        );
        assert!(expand_locale_candidates("C").is_empty());
        assert!(expand_locale_candidates("POSIX").is_empty());
    }

    #[test]
    fn test_all_28_compiled_mo_catalogs_and_translations() {
        let expected_28 = [
            "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "he", "hr", "hu", "is",
            "it", "ja", "ko", "nl", "no", "pl", "pt", "ro", "ru", "sk", "sv", "tr", "uk", "zh",
        ];
        let available = supported_locales();
        for lang in expected_28 {
            assert!(
                available.iter().any(|l| l == lang),
                "Missing compiled .mo catalog for '{lang}'. Found: {available:?}"
            );
        }
        assert!(available.iter().any(|l| l == "pt_BR"));

        assert_eq!(tr_for_locale("pt_BR.UTF-8", "Preferences"), "Preferências");
        assert_eq!(tr_for_locale("es_ES.UTF-8", "Preferences"), "Preferencias");
        assert_eq!(tr_for_locale("fr_FR.UTF-8", "Preferences"), "Préférences");
        assert_eq!(tr_for_locale("de_DE.UTF-8", "Preferences"), "Einstellungen");
        assert_eq!(tr_for_locale("en_US.UTF-8", "Permissão Negada"), "Permission Denied");
        assert_eq!(tr_for_locale("zh_CN.UTF-8", "Permissão Negada"), "权限被拒绝");
        assert_eq!(tr_for_locale("ja_JP.UTF-8", "Permissão Negada"), "アクセス許可が拒否されました");

        // Verify newly ported Rust UI / backend catalog entries across multiple locales
        assert_eq!(tr_for_locale("en_US.UTF-8", "Organizar por"), "Sort by");
        assert_eq!(tr_for_locale("es_ES.UTF-8", "Organizar por"), "Ordenar por");
        assert_eq!(tr_for_locale("fr_FR.UTF-8", "Excluir Pasta"), "Supprimer le dossier");
        assert_eq!(tr_for_locale("de_DE.UTF-8", "Modo do Tema"), "Designmodus");
        assert_eq!(tr_for_locale("ru_RU.UTF-8", "Pasta / Diretório"), "Папка / Каталог");
        assert_eq!(tr_for_locale("zh_CN.UTF-8", "Excluir Pasta"), "删除文件夹");
        assert_eq!(
            tr_for_locale("pt_BR.UTF-8", "Install an RPM package file with verbose progress"),
            "Instalar um arquivo de pacote RPM com progresso detalhado"
        );
    }

    #[test]
    fn test_global_set_locale_and_tr_fmt() {
        let _guard = TEST_LOCALE_LOCK.lock().unwrap();
        let prev = active_locale();

        set_locale("pt_BR.UTF-8");
        assert_eq!(tr("Preferences"), "Preferências");
        assert_eq!(tr(""), "");

        set_locale("es_ES.UTF-8");
        assert_eq!(tr("Preferences"), "Preferencias");
        assert_eq!(
            tr_fmt("Hello {name} ({count})", &[("name", "OnyxSH"), ("count", "28")]),
            "Hello OnyxSH (28)"
        );

        set_locale("C");
        assert_eq!(tr("Preferences"), "Preferences");

        // Restore prior locale
        set_locale(&prev);
    }
}
