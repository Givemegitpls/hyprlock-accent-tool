# hyprlock-wrapper

Rust-бинарь `hyprlock-accent` (`src/`, `cargo build --release`) считает accent-цвет + горизонтальный offset часов для hyprlock на основе текущих обоев awww.

## Реализация (hyprlock-accent)
- Кэш `~/.cache/hyprlock-accent.json`: {md5_пути: {wallpaper, accent, foreground, y_offset}}.
- Нюансы порта (важны при правках): `edge_profile` возвращает `w-1` колонок, `saliency_map` — `w`. `np.convolve(...,'same')` = центрированное окно с делением всегда на K (не trailing/normalized). `fast_image_resize` LANCZOS даёт ±1 младший бит против PIL на краевых пикселях → цвета ±1, y_offset совпадает точно.
- Rust API: `fast_image_resize::images::Image` (не реэкспортирован в корне); resize → `dst.into_vec()` (нет `write_back`); `ResizeAlg::Convolution(FilterType::Lanczos3)`.

## Ключевые решения
- `foreground` — единый цвет формата `RRGGBBDD` (БЕЗ ведущего `#`, alpha DD = 221 ≈0.867). В конфиге: `rgba($foreground)` для gradient-полей, `#$foreground` в Pango-спанах.
- hyprlock типы цветовых полей РАЗНЫЕ: `color`/`font_color`/`inner_color` — тип `color` (принимают `#hex` или `rgba(r,g,b,a)`); `outer_color` (input-field) и `border_color` (shape/image) — тип `gradient` (только `rgba(...)`, НЕ hex, alpha внутри `rgba`; одиночный цвет = `rgba(RRGGBBAA)`). Поэтому 8-значный `#RRGGBBAA` на gradient-поле молча не применяется.
- Pango-разметка внутри `text = cmd[...] echo ...`: в КОНФИГЕ двойной `#` (`##`) = литерал `#` после парсинга hyprlock (первый `#` — escape/не-коммент). Для переменной с цветом без `#` пишут `##$foreground`, чтобы в shell ушло `#$foreground` → Pango видит `#RRGGBBDD`. Просто `#$foreground` ломается: `#` в конфиге = комментарий, режет строку → shell `unexpected EOF`.
- `y_offset` — процент ширины экрана со знаком (`-N%` влево, `+N%` вправо, `0` центр). Выводится именно со знаком процента, т.к. конфиг ожидает `position = $y_offset, 0`.
- Метрика занятости (v2): **saliency_map** = нормализованная сумма двух профилей по колонкам — (а) silhouette-edges (макс горизонтального градиента) и (б) brightness (средняя яркость). Нужна комбинация: edge-only не видит светящиеся/плавные объекты (капля-логотип hypr1), brightness-only размазывает массу. Далее: `_occupied_segments` по порогу 0.35 → свободные промежутки между объектами и краями → колонна в центре самого широкого промежутка (≥28% ширины). Фолбек для плотных обоев (нет промежутка): скользящий min edge-only + edge_penalty 0.1*(dist−0.15).
- Ищется место для вертикальной тёмной колонны (shape) шириной ~28% экрана: saliency-сегментация объектов + центр свободного промежутка; фолбек для плотных.
- Ручной оверрайд: `--set-offset N` пишет `~/.cache/hyprlock-accent.json` (по md5 обоев), дальше скрипт берёт Его вместо авто-вычисления.

## Запуск
```
hyprlock-accent               # вычислить и запустить hyprlock
hyprlock-accent --no-launch   # только вывести WALLPAPER/foreground/y_offset
hyprlock-accent --set-offset -25  # зафиксировать вручную
hyprlock-accent -- -g 2       # флаги hyprlock после --
```

## Окружение
- Сборка: `cargo build --release`; установка `install -Dm755 target/release/hyprlock-accent ~/.local/bin/`.
- Проверка: `cargo check` / `cargo clippy`.
- Гигиена: вывод `y_offset` с `%` обязателен; цвет — 8 hex-символов.
- Обои: `~/.dotfiles/.local/share/backgrounds/` (hypr1..3, kotamota, kotamota2), полноразмер 3840x2160, анализ на downscale 800w.
- Пакетирование: `PKGBUILD` в репозитории (`hyprlock-accent-git`, MIT, cargo build). См. skill `pkgbuild`.

## Пакетирование (makepkg) — важно
- `makepkg` НЕ запускать в корне проекта: он клонирует source (`SRCDEST` = текущая папка) в `./hyprlock-accent/` (bare) и распаковывает в `./src/`, замусоривая настоящий Rust-исходник `src/*.rs`. Собирать в отдельной папке: `mkdir /tmp/pk && cp PKGBUILD /tmp/pk/ && cd /tmp/pk && makepkg -si`.
- Сбои `makepkg` при запуске из корня: `pkgver()` возвращает пустую строку (source-клон не попадает в ожидаемую папку из-за конфликта имён пакета/каталога). Правильное место сборки решает обе проблемы.

## Сетевые
- pip/pypi недоступен напрямую; прокси socks5 в env на 127.0.0.1:1080 (данные у юзера), сборка Rust-зависимостей шла через него.
