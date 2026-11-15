mod build;
mod killfeed;
mod source;
mod verify;

use serde_json::Value;
use std::path::PathBuf;

const HELP: &str = "\
WT Vehicles Parser — база техники для WT Tracker из файлов War Thunder

Использование:
  wt-vehicles-parser [опции]                 собрать vehicles.json
  wt-vehicles-parser verify <файлы|папки>    проверить базу по записям боёв трекера (recordings/)

Опции сборки:
  (по умолчанию данные берутся из https://github.com/gszabi99/War-Thunder-Datamine)
  --game [папка]        вместо датамайна — установленная игра (папка ищется автоматически)
  -o, --out <файл>      куда писать (по умолчанию vehicles.json)
  --overrides <файл>    ручные правки (по умолчанию overrides.json, если есть)
  --langs <список>      языки названий через запятую, первый главный
                        (по умолчанию все языки units.csv: Russian, English, затем остальные)
  --manifest <файл>     записать манифест (версия игры, число записей) — для автообновления в трекере
  --print-version       только вывести версию игры источника и выйти

Опции verify:
  --db <файл>           база для проверки (по умолчанию vehicles.json)
  --game [папка]        брать имена своей техники из игры, а не из датамайна
";

fn arg<'a>(args: &'a [String], names: &[&str]) -> Option<&'a str> {
    args.iter().position(|a| names.contains(&a.as_str())).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn load_data(args: &[String]) -> Result<source::GameData, String> {
    let Some(i) = args.iter().position(|a| a == "--game") else {
        return source::from_datamine();
    };
    let dir = match args.get(i + 1).filter(|a| !a.starts_with("--")) {
        Some(d) => PathBuf::from(d),
        None => source::find_game_dir().ok_or("War Thunder не найден: укажи --game <папка>")?,
    };
    source::from_game(&dir)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if flag(&args, "-h") || flag(&args, "--help") {
        print!("{HELP}");
        return;
    }
    let res = if args.first().map(String::as_str) == Some("verify") {
        cmd_verify(&args[1..])
    } else {
        cmd_build(&args)
    };
    match res {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("Ошибка: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_build(args: &[String]) -> Result<i32, String> {
    let data = load_data(args)?;
    if flag(args, "--print-version") {
        println!("{}", data.version.as_deref().unwrap_or("unknown"));
        return Ok(0);
    }
    eprintln!("Источник: {} (версия {})", data.origin, data.version.as_deref().unwrap_or("?"));
    let wpcost: Value = serde_json::from_str(&data.wpcost).map_err(|e| format!("wpcost: {e}"))?;
    let loc = build::Loc::parse(&data.units_csv)?;
    // every client language: the kill feed shows names in the client's language
    let langs: Vec<String> = match arg(args, &["--langs"]).filter(|l| *l != "all") {
        Some(list) => list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        None => {
            let mut l = vec!["Russian".to_string(), "English".to_string()];
            l.extend(loc.languages().into_iter().filter(|x| x != "Russian" && x != "English"));
            l
        }
    };
    for l in &langs {
        if !loc.has_lang(l) {
            return Err(format!("в units.csv нет языка «{l}»"));
        }
    }
    let unittags: Option<Value> = data.unittags.as_deref().and_then(|t| serde_json::from_str(t).ok());
    if unittags.is_none() {
        eprintln!("unittags недоступен: поле operator будет равно ветке");
    }
    let (mut db, rep) = build::build(&wpcost, unittags.as_ref(), &loc, &langs);

    let ov_path = arg(args, &["--overrides"]).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("overrides.json"));
    if let Ok(text) = std::fs::read_to_string(&ov_path) {
        let ov: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("{}: {e}", ov_path.display()))?;
        let n = build::apply_overrides(&mut db, &ov);
        eprintln!("Ручные правки: {n} из {}", ov_path.display());
    }

    let out = PathBuf::from(arg(args, &["-o", "--out"]).unwrap_or("vehicles.json"));
    let bytes = (serde_json::to_string_pretty(&db).map_err(|e| e.to_string())? + "\n").into_bytes();
    std::fs::write(&out, &bytes).map_err(|e| format!("{}: {e}", out.display()))?;
    if let Some(m) = arg(args, &["--manifest"]) {
        let generated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        use sha2::{Digest, Sha256};
        let sha256: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        // the tracker checks count and sha256 before saving, so a half-updated release is never kept
        let manifest = serde_json::json!({
            "format": 1,
            "game_version": data.version,
            "count": db.len(),
            "sha256": sha256,
            "generated": generated,
            "file": out.file_name().and_then(|f| f.to_str()).unwrap_or("vehicles.json"),
        });
        std::fs::write(m, serde_json::to_string_pretty(&manifest).unwrap() + "\n").map_err(|e| format!("{m}: {e}"))?;
        eprintln!("Манифест: {m}");
    }

    let mut stats: std::collections::BTreeMap<(String, String), usize> = Default::default();
    for v in db.values() {
        let n = v.get("nation").and_then(Value::as_str).unwrap_or("?").to_string();
        let t = v.get("type").and_then(Value::as_str).unwrap_or("?").to_string();
        *stats.entry((n, t)).or_default() += 1;
    }
    println!("Юнитов: {}, имён в базе: {} → {}", rep.units, db.len(), out.display());
    let mut cur = String::new();
    for ((n, t), c) in &stats {
        if *n != cur {
            print!("{}  {n}:", if cur.is_empty() { "" } else { "\n" });
            cur = n.clone();
        }
        print!(" {t} {c}");
    }
    println!();
    if !rep.ambiguous_nation.is_empty() {
        println!("Одно имя у нескольких наций ({}): {}", rep.ambiguous_nation.len(), rep.ambiguous_nation.join(", "));
    }
    if !rep.ambiguous_type.is_empty() {
        println!("Одно имя у разных типов: {}", rep.ambiguous_type.join(", "));
    }
    if !rep.missing_loc.is_empty() {
        println!("Без локализованного имени: {}", rep.missing_loc.join(", "));
    }
    Ok(0)
}

fn cmd_verify(args: &[String]) -> Result<i32, String> {
    let db = PathBuf::from(arg(args, &["--db"]).unwrap_or("vehicles.json"));
    let skip_next = ["--db", "--game"];
    let mut inputs = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        // --game has an optional value
        let takes_value = a == "--db"
            || (a == "--game" && args.get(i + 1).is_some_and(|n| PathBuf::from(n).join("lang.vromfs.bin").is_file()));
        if skip_next.contains(&a.as_str()) && takes_value {
            i += 2;
            continue;
        }
        if !a.starts_with("--") {
            inputs.push(PathBuf::from(a));
        }
        i += 1;
    }
    let data = load_data(args).map_err(|e| eprintln!("(своя техника не проверяется: {e})")).ok();
    let loc = data.as_ref().map(|d| build::Loc::parse(&d.units_csv)).transpose()?;
    let ok = verify::run(&db, &inputs, loc.as_ref())?;
    Ok(if ok { 0 } else { 2 })
}
