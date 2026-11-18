# WT Vehicles Parser

**English** | [Русский](README.ru.md)

Builds `vehicles.json` for [WT Tracker](https://github.com/mrvokintos/WT-Tracker) from [War-Thunder-Datamine](https://github.com/gszabi99/War-Thunder-Datamine).

Nation and vehicle class are taken from `char.vromfs.bin_u/config/wpcost.blkx`, names from `lang.vromfs.bin_u/lang/units.csv`. The parser uses the short name `<id>_1`, which is what the kill feed displays, so names match `localhost:8111/hudmsg` exactly. Names are included in all client languages, because the kill feed uses the client's language. The previous wiki-based database had about a quarter of the names wrong.

## Usage

```bash
cargo build --release
./target/release/wt-vehicles-parser                 # from the datamine, writes vehicles.json
./target/release/wt-vehicles-parser --game          # from a local install (if the datamine is behind a patch)
./target/release/wt-vehicles-parser -o out.json --manifest manifest.json
```

Without a path, `--game` looks for the game in Steam libraries and the Gaijin launcher folder. The output is identical to a datamine build.

## CI

Once a day (03:17 UTC) `update-db.yml` compares the datamine `version` with `db/manifest.json` in the repository. When a new patch is out, it builds the database, commits `vehicles.json` and `db/manifest.json`, and replaces the files of the `db-latest` release. The tracker downloads them from `releases/download/db-latest/`, so other releases in this repository don't affect it. The commits also keep GitHub from disabling the schedule after 60 days without activity.

Manual run: Actions → update-db → Run (the force option rebuilds even if the version is unchanged).

## Checking against battle recordings

The tracker can save the raw kill feed to `recordings/`. To find names missing from the database:

```bash
./target/release/wt-vehicles-parser verify ../WT-Tracker/recordings
```

## Format

```json
{
  "M1A1 SA": { "nation": "USA", "type": "tank", "id": "us_m1a1_aim_abrams", "operator": "australia" },
  "F-16C":   { "nation": "unknown", "nations": ["USA", "Israel"], "type": "plane", "id": "f_16c_block_40_barak_2" }
}
```

`operator` is the flag the scoreboard shows for the vehicle: `operatorCountry` from `char.vromfs.bin_u/config/unittags.blkx`, or the tree's own flag (`ussr`, `germany`...). Names shared by vehicles with different flags get an `operators` list instead. `type` is one of `tank`, `zsu`, `heli` or `plane`; ships are not included. The `nations` field is added when the same name exists in several trees, and `ids` lists every vehicle that shares the name (`id` holds the first one). The file has one entry per name and language, so it contains about four times more entries than vehicles; WT Tracker counts unique ids.

Manual corrections go into `overrides.json`: listed fields are replaced, `null` removes the entry.

```json
{ "M1A1 SA": { "note": "Australian" } }
```

### names.json

`--names names.json` also writes a map from vehicle id to its tech tree name (`<id>_shop`, falling back to `<id>_1`), with game-font marker glyphs removed. It is used by WT Discord RPC. `English` is always present; other languages are listed only when they differ from it. Lowercase keys hold the tech tree (`nation`) and the vehicle class from `unittags.blkx` (`class`: `medium_tank`, `fighter`, `destroyer`...).

```json
{ "ussr_t_80ue1": { "English": "T-80U-E1", "Russian": "Т-80У-Е1", "nation": "ussr", "class": "medium_tank" } }
```

## License

[GPL-3.0](LICENSE)
