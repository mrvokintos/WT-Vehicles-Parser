// kill feed uses the short name, i.e. units.csv `<id>_1`

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const NATIONS: &[(&str, &str)] = &[
    ("country_usa", "USA"),
    ("country_germany", "Germany"),
    ("country_ussr", "USSR"),
    ("country_britain", "Great Britain"),
    ("country_japan", "Japan"),
    ("country_china", "China"),
    ("country_italy", "Italy"),
    ("country_france", "France"),
    ("country_sweden", "Sweden"),
    ("country_israel", "Israel"),
];

pub fn map_class(class: &str) -> Option<&'static str> {
    Some(match class {
        "exp_tank" | "exp_tank_destroyer" | "exp_heavy_tank" => "tank",
        "exp_SPAA" => "zsu",
        "exp_helicopter" => "heli",
        "exp_fighter" | "exp_assault" | "exp_bomber" => "plane",
        _ => return None,
    })
}

const TYPE_PRIORITY: &[&str] = &["tank", "zsu", "heli", "plane"];

fn nation_rank(n: &str) -> usize {
    NATIONS.iter().position(|(_, v)| *v == n).unwrap_or(usize::MAX)
}

pub struct Loc {
    cols: HashMap<String, usize>,
    rows: HashMap<String, Vec<String>>,
}

impl Loc {
    pub fn parse(csv_text: &str) -> Result<Self, String> {
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(b';')
            .flexible(true)
            .has_headers(true)
            .from_reader(csv_text.trim_start_matches('\u{feff}').as_bytes());
        let headers = rdr.headers().map_err(|e| e.to_string())?.clone();
        let cols = headers
            .iter()
            .enumerate()
            .map(|(i, h)| (h.trim_matches(|c| c == '<' || c == '>').split('|').next().unwrap_or("").to_string(), i))
            .collect();
        let mut rows = HashMap::new();
        for rec in rdr.records() {
            let rec = rec.map_err(|e| e.to_string())?;
            if let Some(id) = rec.get(0) {
                rows.insert(id.to_string(), rec.iter().map(str::to_string).collect());
            }
        }
        Ok(Self { cols, rows })
    }

    pub fn get(&self, id: &str, lang: &str) -> Option<&str> {
        let col = *self.cols.get(lang)?;
        self.rows.get(id)?.get(col).map(String::as_str).filter(|s| !s.trim().is_empty())
    }

    pub fn has_lang(&self, lang: &str) -> bool {
        self.cols.contains_key(lang)
    }

    /// Language columns in file order.
    pub fn languages(&self) -> Vec<String> {
        let mut l: Vec<(&String, &usize)> =
            self.cols.iter().filter(|(k, _)| !matches!(k.as_str(), "ID" | "Comments" | "max_chars" | "")).collect();
        l.sort_by_key(|(_, i)| **i);
        l.into_iter().map(|(k, _)| k.clone()).collect()
    }
}

#[derive(Default)]
struct Entry {
    nations: BTreeSet<(usize, String)>,
    types: BTreeSet<&'static str>,
    ids: Vec<String>,
    operators: Vec<String>,
}

/// Names as the tracker sees them: zero-width spaces dropped (the tracker strips them from the
/// kill feed too), and the Japanese "\t" line-break hint rendered as a space or as nothing.
fn name_variants(raw: &str) -> Vec<String> {
    let s = raw.replace('\u{200b}', "");
    let mut out = vec![];
    for v in [s.replace("\\t", " ").replace('\t', " "), s.replace("\\t", "").replace('\t', "")] {
        let v = v.split(' ').filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ");
        if !v.is_empty() && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

pub struct BuildReport {
    pub units: usize,
    pub names: usize,
    pub ambiguous_nation: Vec<String>,
    pub ambiguous_type: Vec<String>,
    pub missing_loc: Vec<String>,
}

/// Flag shown for the vehicle on the scoreboard: operatorCountry from unittags, or the tree's flag.
fn operator(unittags: Option<&Value>, id: &str, tree_country: &str) -> String {
    let op = unittags
        .and_then(|t| t.get(id))
        .and_then(|u| u.get("operatorCountry"))
        .and_then(Value::as_str)
        .filter(|o| *o != "country_invisible")
        .unwrap_or(tree_country);
    op.trim_start_matches("country_").to_string()
}

pub fn build(wpcost: &Value, unittags: Option<&Value>, loc: &Loc, langs: &[String]) -> (BTreeMap<String, Value>, BuildReport) {
    let country: HashMap<&str, &str> = NATIONS.iter().copied().collect();
    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    let mut name_lang: HashMap<String, usize> = HashMap::new();
    let mut report = BuildReport { units: 0, names: 0, ambiguous_nation: vec![], ambiguous_type: vec![], missing_loc: vec![] };

    let Some(obj) = wpcost.as_object() else { return (BTreeMap::new(), report) };
    let mut units: Vec<(&String, &'static str, &str)> = Vec::new();
    let mut operators: HashMap<&String, String> = HashMap::new();
    for (id, v) in obj {
        let (Some(class), Some(c)) = (v.get("unitClass").and_then(Value::as_str), v.get("country").and_then(Value::as_str)) else {
            continue;
        };
        if let (Some(vtype), Some(nation)) = (map_class(class), country.get(c)) {
            units.push((id, vtype, nation));
            operators.insert(id, operator(unittags, id, c));
        }
    }
    units.sort();
    report.units = units.len();

    let mut named: std::collections::HashSet<&String> = Default::default();
    // separate pass per language so english aliases can't steal russian names
    for (li, lang) in langs.iter().enumerate() {
        for &(id, vtype, nation) in &units {
            let Some(raw) = loc.get(&format!("{id}_1"), lang) else { continue };
            named.insert(id);
            for name in name_variants(raw) {
                let owner = *name_lang.entry(name.clone()).or_insert(li);
                if owner < li && !entries.get(&name).is_some_and(|e| e.ids.contains(id)) {
                    continue;
                }
                let e = entries.entry(name).or_default();
                e.nations.insert((nation_rank(nation), nation.to_string()));
                e.types.insert(vtype);
                if !e.ids.contains(id) {
                    e.ids.push(id.clone());
                }
                let op = &operators[id];
                if !e.operators.contains(op) {
                    e.operators.push(op.clone());
                }
            }
        }
    }
    report.missing_loc = units.iter().filter(|(id, _, _)| !named.contains(id)).map(|(id, _, _)| id.to_string()).collect();

    let mut out = BTreeMap::new();
    for (name, e) in entries {
        let nations: Vec<&str> = e.nations.iter().map(|(_, n)| n.as_str()).collect();
        let vtype = TYPE_PRIORITY.iter().find(|t| e.types.contains(*t)).copied().unwrap_or("special");
        if e.types.len() > 1 {
            report.ambiguous_type.push(name.clone());
        }
        let mut m = Map::new();
        if nations.len() == 1 {
            m.insert("nation".into(), json!(nations[0]));
        } else {
            report.ambiguous_nation.push(name.clone());
            m.insert("nation".into(), json!("unknown"));
            m.insert("nations".into(), json!(nations));
        }
        m.insert("type".into(), json!(vtype));
        m.insert("id".into(), json!(e.ids[0]));
        // one name shared by several vehicles (MQ-1 in many trees): keep all of them, so counts stay right
        if e.ids.len() > 1 {
            m.insert("ids".into(), json!(e.ids));
        }
        if e.operators.len() == 1 {
            m.insert("operator".into(), json!(e.operators[0]));
        } else if !e.operators.is_empty() {
            m.insert("operators".into(), json!(e.operators));
        }
        out.insert(name, Value::Object(m));
    }
    report.names = out.len();
    (out, report)
}

/// Game-font glyphs (premium/squadron/nation markers) that don't render outside the game.
fn is_marker(c: char) -> bool {
    matches!(c,
        '\u{2400}'..='\u{243f}' // control pictures: ␗ ␙ ␠
        | '\u{2580}'..='\u{259f}' // block elements: ▄ ▀ ▂
        | '\u{25a0}'..='\u{25ff}' // geometric shapes: ◄ ◊ ◔
        | '\u{2622}' | '\u{22e0}'
        | '\u{e000}'..='\u{f8ff}' // private use
        | '\u{200b}')
}

/// Name for display outside the game (Discord Rich Presence): no marker glyphs, single spaces.
pub fn display_name(raw: &str) -> Option<String> {
    let s: String = raw.replace("\\t", " ").replace('\t', " ").chars().filter(|c| !is_marker(*c)).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    (!s.is_empty()).then_some(s)
}

/// Most specific first: an MBT is tagged type_medium_tank, a strike jet both
/// type_fighter and type_strike_aircraft.
const CLASS_PRIORITY: &[&str] = &[
    "heavy_tank", "tank_destroyer", "spaa", "light_tank", "medium_tank",
    "attack_helicopter", "utility_helicopter",
    "bomber", "strike_aircraft", "assault", "fighter",
    "battleship", "battlecruiser", "heavy_cruiser", "light_cruiser", "destroyer", "frigate",
    "submarine_chaser", "torpedo_boat", "gun_boat", "armored_boat", "barge", "boat",
];

/// Vehicle class from unittags `type_*` tags.
fn class(unittags: Option<&Value>, id: &str) -> Option<&'static str> {
    let tags = unittags?.get(id)?.get("tags")?.as_object()?;
    CLASS_PRIORITY.iter().find(|c| tags.contains_key(&format!("type_{c}"))).copied()
}

/// `id → {language → name}` for every unit in wpcost: the tech tree name (`<id>_shop`),
/// falling back to the short one (`<id>_1`). Only English is always present, other
/// languages are listed when they differ from it. Lowercase keys carry extra data:
/// `nation` (tech tree: usa, ussr...) and `class` (medium_tank, fighter...).
pub fn names(wpcost: &Value, unittags: Option<&Value>, loc: &Loc) -> BTreeMap<String, BTreeMap<String, String>> {
    let langs = loc.languages();
    let mut out = BTreeMap::new();
    let Some(obj) = wpcost.as_object() else { return out };
    for (id, v) in obj {
        if v.get("unitClass").and_then(Value::as_str).is_none() {
            continue;
        }
        let name = |lang: &str| {
            [format!("{id}_shop"), format!("{id}_1")].iter().find_map(|k| loc.get(k, lang).and_then(display_name))
        };
        let Some(en) = name("English") else { continue };
        let mut m = BTreeMap::new();
        for l in &langs {
            if let Some(n) = name(l).filter(|n| *n != en) {
                m.insert(l.clone(), n);
            }
        }
        m.insert("English".to_string(), en);
        if let Some(c) = v.get("country").and_then(Value::as_str) {
            m.insert("nation".to_string(), c.trim_start_matches("country_").to_string());
        }
        if let Some(c) = class(unittags, id) {
            m.insert("class".to_string(), c.to_string());
        }
        out.insert(id.clone(), m);
    }
    out
}

pub fn apply_overrides(db: &mut BTreeMap<String, Value>, overrides: &Value) -> usize {
    let Some(o) = overrides.as_object() else { return 0 };
    for (name, v) in o {
        if name.starts_with("//") {
            continue;
        }
        match v {
            Value::Null => {
                db.remove(name);
            }
            Value::Object(fields) => {
                let e = db.entry(name.clone()).or_insert_with(|| json!({}));
                if let Some(eo) = e.as_object_mut() {
                    for (k, fv) in fields {
                        eo.insert(k.clone(), fv.clone());
                    }
                }
            }
            _ => {}
        }
    }
    o.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "\"<ID|readonly|noverify>\";\"<English>\";\"<Russian>\"\n\
        \"us_m1a1_aim_abrams_1\";\"M1A1 SA\";\"M1A1 SA\"\n\
        \"ussr_t_80ue1_1\";\"T-80UE-1\";\"Т-80У-Е1\"\n\
        \"us_mq_1_1\";\"MQ-1\";\"MQ-1\"\n\
        \"germ_mq_1_1\";\"MQ-1\";\"MQ-1\"\n\
        \"us_ship_1\";\"Ship\";\"Корабль\"\n";

    fn wpcost() -> Value {
        json!({
            "economicRankMax": 42,
            "us_m1a1_aim_abrams": {"country": "country_usa", "unitClass": "exp_tank"},
            "ussr_t_80ue1": {"country": "country_ussr", "unitClass": "exp_tank"},
            "us_mq_1": {"country": "country_usa", "unitClass": "exp_assault"},
            "germ_mq_1": {"country": "country_germany", "unitClass": "exp_assault"},
            "us_ship": {"country": "country_usa", "unitClass": "exp_destroyer"},
        })
    }

    #[test]
    fn builds_killfeed_names() {
        let loc = Loc::parse(CSV).unwrap();
        let (db, rep) = build(&wpcost(), None, &loc, &["Russian".into(), "English".into()]);
        assert_eq!(db["Т-80У-Е1"], json!({"nation": "USSR", "type": "tank", "id": "ussr_t_80ue1", "operator": "ussr"}));
        assert_eq!(db["T-80UE-1"]["nation"], "USSR", "английский алиас");
        assert_eq!(db["M1A1 SA"]["nation"], "USA");
        assert_eq!(db["MQ-1"]["nations"], json!(["USA", "Germany"]));
        assert_eq!(db["MQ-1"]["nation"], "unknown");
        assert!(!db.contains_key("Корабль"));
        assert_eq!(rep.units, 4);
    }

    #[test]
    fn name_variants_match_tracker_normalization() {
        assert_eq!(name_variants("▄\u{200b}M44"), vec!["▄M44"]);
        assert_eq!(name_variants("▄オサー\\tAK"), vec!["▄オサー AK", "▄オサーAK"]);
        assert_eq!(name_variants(" M1A1 SA "), vec!["M1A1 SA"]);
    }

    #[test]
    fn operator_flags() {
        let loc = Loc::parse(CSV).unwrap();
        let tags = json!({"us_m1a1_aim_abrams": {"operatorCountry": "country_australia"},
                          "ussr_t_80ue1": {"operatorCountry": "country_invisible"}});
        let (db, _) = build(&wpcost(), Some(&tags), &loc, &["Russian".into()]);
        assert_eq!(db["M1A1 SA"]["operator"], "australia");
        assert_eq!(db["Т-80У-Е1"]["operator"], "ussr", "no flag -> tree flag");
        assert_eq!(db["MQ-1"]["operators"], json!(["germany", "usa"]));
        assert_eq!(db["MQ-1"]["ids"], json!(["germ_mq_1", "us_mq_1"]));
    }

    #[test]
    fn names_by_id() {
        let csv = "\"<ID|readonly|noverify>\";\"<English>\";\"<Russian>\"\n\
            \"us_m1a1_aim_abrams_shop\";\"M1A1 SA\";\"M1A1 SA\"\n\
            \"us_m1a1_aim_abrams_0\";\"Tank, Combat, Full Tracked\";\"Танк\"\n\
            \"us_m1a1_aim_abrams_1\";\"M1A1 SA\";\"M1A1 SA\"\n\
            \"ussr_t_80ue1_1\";\"▄T-80UE-1\";\"▄Т-80У-Е1\"\n\
            \"us_mq_1_shop\";\"\u{e059} MQ-1\";\"\"\n";
        let tags = json!({"us_m1a1_aim_abrams": {"tags": {"type_tank": true, "type_medium_tank": true}},
                          "us_mq_1": {"tags": {"type_fighter": true, "type_strike_aircraft": true}}});
        let n = names(&wpcost(), Some(&tags), &Loc::parse(csv).unwrap());
        assert_eq!(
            n["us_m1a1_aim_abrams"],
            BTreeMap::from([
                ("English".into(), "M1A1 SA".into()),
                ("nation".into(), "usa".into()),
                ("class".into(), "medium_tank".into())
            ])
        );
        assert_eq!(n["us_mq_1"]["class"], "strike_aircraft");
        assert!(!n["ussr_t_80ue1"].contains_key("class"));
        assert_eq!(n["ussr_t_80ue1"]["English"], "T-80UE-1");
        assert_eq!(n["ussr_t_80ue1"]["Russian"], "Т-80У-Е1");
        assert!(!n["us_mq_1"].contains_key("Russian"), "empty russian is skipped");
        assert_eq!(n["us_mq_1"]["English"], "MQ-1");
        assert!(!n.contains_key("germ_mq_1"));
        assert!(!n.contains_key("economicRankMax"));
    }

    #[test]
    fn languages_in_file_order() {
        let loc = Loc::parse(CSV).unwrap();
        assert_eq!(loc.languages(), vec!["English", "Russian"]);
    }

    #[test]
    fn overrides_merge_and_delete() {
        let loc = Loc::parse(CSV).unwrap();
        let (mut db, _) = build(&wpcost(), None, &loc, &["Russian".into()]);
        apply_overrides(&mut db, &json!({"M1A1 SA": {"note": "тест"}, "MQ-1": null}));
        assert_eq!(db["M1A1 SA"]["note"], "тест");
        assert_eq!(db["M1A1 SA"]["nation"], "USA");
        assert!(!db.contains_key("MQ-1"));
    }
}
