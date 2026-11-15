# WT Vehicles Parser

[English](README.md) | **Русский**

Собирает `vehicles.json` для [WT Tracker](https://github.com/mrvokintos/WT-Tracker) из [War-Thunder-Datamine](https://github.com/gszabi99/War-Thunder-Datamine).

Нация и класс техники берутся из `char.vromfs.bin_u/config/wpcost.blkx`, названия — из `lang.vromfs.bin_u/lang/units.csv`. Используется короткое название `<id>_1`: именно оно выводится в киллфиде, поэтому названия в базе полностью совпадают с `localhost:8111/hudmsg`. Названия включены на всех языках клиента, потому что киллфид выводится на языке клиента. В прежней базе, собранной с вики, расходилась примерно четверть названий.

## Использование

```bash
cargo build --release
./target/release/wt-vehicles-parser                 # из датамайна, результат в vehicles.json
./target/release/wt-vehicles-parser --game          # из установленной игры (если датамайн ещё не обновлён после патча)
./target/release/wt-vehicles-parser -o out.json --manifest manifest.json
```

Без указания пути `--game` ищет игру в библиотеках Steam и в папке лаунчера Gaijin. Результат совпадает со сборкой из датамайна.

## CI

`update-db.yml` каждые 6 часов сравнивает `version` датамайна с `db/manifest.json` в репозитории. После выхода патча он собирает базу, коммитит `vehicles.json` и `db/manifest.json` и заменяет файлы релиза `db-latest`. Трекер загружает их по ссылке `releases/download/db-latest/`, поэтому другие релизы в этом репозитории на него не влияют. Коммиты также не дают GitHub отключить расписание после 60 дней без активности.

Ручной запуск: Actions → update-db → Run (опция force пересобирает базу без смены версии).

## Проверка по записям боёв

Трекер может сохранять необработанный киллфид в `recordings/`. Найти названия, которых нет в базе:

```bash
./target/release/wt-vehicles-parser verify ../WT-Tracker/recordings
```

## Формат

```json
{
  "M1A1 SA": { "nation": "USA", "type": "tank", "id": "us_m1a1_aim_abrams", "operator": "australia" },
  "F-16C":   { "nation": "unknown", "nations": ["USA", "Israel"], "type": "plane", "id": "f_16c_block_40_barak_2" }
}
```

`operator` — флаг, который таблица счёта показывает для машины: `operatorCountry` из `char.vromfs.bin_u/config/unittags.blkx` или флаг её ветки (`ussr`, `germany`…). Если одно название носят машины с разными флагами, вместо него записывается список `operators`. `type` принимает значения `tank`, `zsu`, `heli` и `plane`, флот не включается. Поле `nations` добавляется, если одно название встречается в нескольких ветках, а `ids` перечисляет все машины с этим названием (в `id` — первая из них). В файле по записи на каждое название и язык, поэтому записей примерно вчетверо больше, чем техники; WT Tracker считает уникальные `id`.

Ручные правки задаются в `overrides.json`: указанные поля заменяются, `null` удаляет запись.

```json
{ "M1A1 SA": { "note": "Австралийский" } }
```

## Лицензия

[GPL-3.0](LICENSE)
