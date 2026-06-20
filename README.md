## Создание и обновление базы данных техники War Thunder

### Автоматическое обновление с Wiki

1. Скачайте HTML страницы с War Thunder Wiki:
   - Наземная техника → сохранить как `wiki/ground.htm`
   - Авиация → сохранить как `wiki/aviation.htm`
   - Вертолёты → сохранить как `wiki/helicopters.htm`

2. Запустите парсер:
   ```bash
   python parse_vehicles.py
   ```

3. Парсер автоматически создаст:
   - `vehicles.json` - общая база всей техники (в корне)
   - `wiki/ground_vehicles.json` - только наземная техника
   - `wiki/aviation_vehicles.json` - только авиация
   - `wiki/helicopters_vehicles.json` - только вертолёты

### Ручное добавление техники

Добавьте запись в `vehicles.json` вручную:
```json
{
  "Название техники": {
    "nation": "USA",
    "type": "tank"
  }
}
```

---
