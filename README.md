# hyprlock-accent

Компute accent-цвет, foreground-цвет и горизонтальный offset часов для
[hyprlock](https://github.com/hyprwm/hyprlock) на основе текущих обоев `awww`.

Rust-версия. Скорость: ~3-4 ms с прогретым кэшем, ~110 ms холодный запуск
(декод+анализ).

## Как работает

1. Читает путь обоев из `awww query`.
2. Делит обои на объекты (saliency = края + яркость), находит самый
   широкий свободный промежуток и центрует в нём вертикальную колонну часов
   (28% ширины). `y_offset` — процент со знаком (0 = центр, влево/вправо).
3. Акцент = самый яркий+насыщенный цвет, foreground = самый яркий (не
   белый), оба `RRGGBBFF` без ведущего `#`.
4. Экспортирует `WALLPAPER`/`accent`/`foreground`/`y_offset` в окружение и
   запускает `hyprlock`.

Результат кэшируется в `~/.cache/hyprlock-accent.json` по md5 пути обоев;
пересчёт только при смене обоев.

## Сборка и установка

```sh
cargo build --release
# бинарник: target/release/hyprlock-accent
install -Dm755 target/release/hyprlock-accent ~/.local/bin/hyprlock-accent
```

Для Arch Linux есть `PKGBUILD` в этом репозитории (пакет `hyprlock-accent-git`):

```sh
makepkg -si
```

В конфиге hyprlock переменные подставляются как `$accent`, `$foreground`,
`$y_offset`, `$WALLPAPER`.

## Использование

```sh
hyprlock-accent          # вычислить и запустить hyprlock
hyprlock-accent --no-launch        # только вывести значения
hyprlock-accent --set-offset -25   # зафиксировать offset вручную
hyprlock-accent -- -g 2            # флаги hyprlock после --
hyprlock-accent --max-width 1200 --column-frac 0.3
```

Переменные окружения:

- `HYPRLOCK_WALLPAPER` — принудительный путь обоев (обход `awww query`).
- `HYPRLOCK_ACCENT` / `HYPRLOCK_FOREGROUND` — ручной оверрайд цвета
  (`RRGGBB` или `RRGGBBAA`, без `#`).

## Формат цветов (подробности из истории)

- `foreground`/`accent` — 8 hex (`RRGGBBAA`), БЕЗ ведущего `#`.
- Поля hyprlock бывают двух типов: `color` (принимают `#hex` или
  `rgba(...)`) и `gradient` (только `rgba(...)`, alpha внутри). `#RRGGBBAA`
  на gradient-поле молча не применяется.
- В Pango-разметке внутри `text = cmd[...] echo ...` пишут `##$acc`:
  двойной `#` в конфиге = литерал `#` после парсинга hyprlock.
