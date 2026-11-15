use std::io::Read;
use std::path::{Path, PathBuf};
use wt_blk::vromf::{BlkOutputFormat, File, FileFilter, VromfUnpacker};

pub const DATAMINE_RAW: &str = "https://raw.githubusercontent.com/gszabi99/War-Thunder-Datamine/master";

pub struct GameData {
    pub wpcost: String,
    pub units_csv: String,
    /// config/unittags.blk as JSON (operatorCountry = flag on the scoreboard)
    pub unittags: Option<String>,
    pub origin: String,
    pub version: Option<String>,
}

pub fn from_game(dir: &Path) -> Result<GameData, String> {
    let (wpcost, version) = unpack(&dir.join("char.vromfs.bin"), "config/wpcost.blk")?;
    let (units_csv, _) = unpack(&dir.join("lang.vromfs.bin"), "lang/units.csv")?;
    let unittags = unpack(&dir.join("char.vromfs.bin"), "config/unittags.blk").ok().map(|u| u.0);
    Ok(GameData { wpcost, units_csv, unittags, origin: format!("игра: {}", dir.display()), version })
}

fn unpack(vromf: &Path, inner: &str) -> Result<(String, Option<String>), String> {
    let file = File::new(vromf).map_err(|e| format!("{}: {e}", vromf.display()))?;
    let un = VromfUnpacker::from_file(&file, false).map_err(|e| format!("{}: {e}", vromf.display()))?;
    let version = un.latest_version().ok().flatten().map(|v| v.to_string());
    let out = un
        .unpack_one(Path::new(inner), Some(BlkOutputFormat::Json), true, FileFilter::All)
        .map_err(|e| format!("{} → {inner}: {e}", vromf.display()))?;
    let (_, buf) = out.split();
    let text = String::from_utf8(buf).map_err(|e| format!("{inner}: не UTF-8: {e}"))?;
    Ok((text, version))
}

pub fn from_datamine() -> Result<GameData, String> {
    let get = |path: &str| -> Result<String, String> {
        let url = format!("{DATAMINE_RAW}/{path}");
        eprintln!("Скачиваю {url}");
        let resp = ureq::get(&url).call().map_err(|e| format!("{url}: {e}"))?;
        let mut s = String::new();
        resp.into_reader().read_to_string(&mut s).map_err(|e| format!("{url}: {e}"))?;
        Ok(s)
    };
    Ok(GameData {
        wpcost: get("char.vromfs.bin_u/config/wpcost.blkx")?,
        units_csv: get("lang.vromfs.bin_u/lang/units.csv")?,
        unittags: get("char.vromfs.bin_u/config/unittags.blkx").ok(),
        origin: format!("датамайн: {DATAMINE_RAW}"),
        version: get("version").ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()),
    })
}

pub fn find_game_dir() -> Option<PathBuf> {
    candidates().into_iter().find(|d| d.join("lang.vromfs.bin").is_file() && d.join("char.vromfs.bin").is_file())
}

fn candidates() -> Vec<PathBuf> {
    let mut steam_roots: Vec<PathBuf> = Vec::new();
    let mut out: Vec<PathBuf> = Vec::new();
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    if let Some(h) = &home {
        for p in [
            ".local/share/Steam",
            ".steam/steam",
            ".steam/root",
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
        ] {
            steam_roots.push(h.join(p));
        }
        out.push(h.join("WarThunder"));
        out.push(h.join(".local/share/WarThunder"));
        out.push(h.join("Games/WarThunder"));
    }
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(p) = std::env::var_os(var) {
            steam_roots.push(PathBuf::from(p).join("Steam"));
        }
    }
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        out.push(PathBuf::from(p).join("WarThunder"));
    }
    let re = regex::Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
    let mut libs: Vec<PathBuf> = steam_roots.clone();
    for root in &steam_roots {
        if let Ok(vdf) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            libs.extend(re.captures_iter(&vdf).map(|c| PathBuf::from(c[1].replace("\\\\", "\\"))));
        }
    }
    let mut res: Vec<PathBuf> = libs.iter().map(|l| l.join("steamapps/common/War Thunder")).collect();
    res.extend(out);
    res
}
