use crate::build::Loc;
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub fn normalize(s: &str) -> String {
    s.replace('\u{a0}', " ").trim().to_lowercase()
}

pub fn vehicles_in(msg: &str) -> Vec<String> {
    let re = Regex::new(r"^(.+?)\s\((.+)\)$").unwrap();
    let ai = Regex::new(r"(?i)\[(ии|ai)\]\s*").unwrap();
    let veh = |part: &str| -> Option<String> {
        let c = re.captures(part.trim())?;
        let v = c[2].trim();
        let up = v.to_uppercase();
        if up == "СОУ" || up == "СОЦ" {
            return Some(part.trim().to_string());
        }
        Some(ai.replace_all(v, "").trim().to_string())
    };
    match crate::killfeed::parse(msg) {
        Some(crate::killfeed::Event::Kill { killer, victim }) => [veh(&killer), veh(&victim)].into_iter().flatten().collect(),
        Some(crate::killfeed::Event::Death { who }) => veh(&who).into_iter().collect(),
        None => vec![],
    }
}

fn collect_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(p) {
                let mut v: Vec<_> = rd.flatten().map(|e| e.path()).filter(|f| f.extension().is_some_and(|e| e == "jsonl")).collect();
                v.sort();
                out.extend(v);
            }
        } else {
            out.push(p.clone());
        }
    }
    out
}

pub fn run(db_path: &Path, inputs: &[PathBuf], loc: Option<&Loc>) -> Result<bool, String> {
    let text = std::fs::read_to_string(db_path).map_err(|e| format!("{}: {e}", db_path.display()))?;
    let db: serde_json::Map<String, Value> =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let exact: HashSet<&str> = db.keys().map(String::as_str).collect();
    let norm: HashSet<String> = db.keys().map(|k| normalize(k)).collect();

    let files = collect_files(inputs);
    if files.is_empty() {
        return Err("нет файлов *.jsonl".into());
    }
    let (mut total, mut hit_exact, mut hit_norm) = (0usize, 0usize, 0usize);
    let mut missing: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut own_checks: Vec<(String, String, bool)> = Vec::new();

    for f in &files {
        let content = std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?;
        let mut msgs = Vec::new();
        let mut owns = Vec::new();
        for line in content.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            match v.get("kind").and_then(Value::as_str) {
                Some("dmg") => msgs.push(v.get("msg").and_then(Value::as_str).unwrap_or("").to_string()),
                Some("own") => owns.push(v.get("type").and_then(Value::as_str).unwrap_or("").to_string()),
                _ => {}
            }
        }
        for m in &msgs {
            for name in vehicles_in(m) {
                total += 1;
                seen.insert(name.clone());
                if exact.contains(name.as_str()) {
                    hit_exact += 1;
                } else if norm.contains(&normalize(&name)) {
                    hit_norm += 1;
                } else {
                    *missing.entry(name).or_default() += 1;
                }
            }
        }
        if let Some(loc) = loc {
            for t in owns {
                let id = t.rsplit('/').next().unwrap_or(&t).to_string();
                let expected = loc.get(&format!("{id}_1"), "Russian").unwrap_or("?").to_string();
                let found = msgs.iter().any(|m| m.contains(&format!("({expected})")));
                own_checks.push((id, expected, found));
            }
        }
    }

    println!("Файлов: {}, упоминаний техники: {total}, уникальных имён: {}", files.len(), seen.len());
    println!("  точное совпадение: {hit_exact}");
    println!("  без учёта регистра/пробелов: {hit_norm}");
    println!("  не найдено: {} ({} уникальных)", missing.values().sum::<usize>(), missing.len());
    let mut miss: Vec<_> = missing.iter().collect();
    miss.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (name, n) in miss {
        println!("    {n:>4} × {name}");
    }
    if !own_checks.is_empty() {
        println!("Своя техника (/indicators → <id>_1 в киллфиде):");
        let mut by: HashMap<&str, (&str, bool)> = HashMap::new();
        for (id, exp, ok) in &own_checks {
            let e = by.entry(id).or_insert((exp, false));
            e.1 |= *ok;
        }
        for (id, (exp, ok)) in by {
            println!("    {} {id} → «{exp}»", if ok { "✅" } else { "— (не встретилась в логе)" });
        }
    }
    Ok(missing.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_names() {
        assert_eq!(
            vehicles_in("=FUGAS= Etego (Leopard 2RI) уничтожил ^1SQD^ arey911 (Leopard 2A4M)"),
            vec!["Leopard 2RI", "Leopard 2A4M"]
        );
        assert_eq!(vehicles_in("Bot ([ИИ] Панцирь-С1) сбил Joe (F-16C)"), vec!["Панцирь-С1", "F-16C"]);
        assert_eq!(vehicles_in("Joe (F-16C) разбился"), vec!["F-16C"]);
        assert!(vehicles_in("[HARBS] mrvokintos (M1A1 SA) получил \"Помощь с починкой: x2\"").is_empty());
    }
}
