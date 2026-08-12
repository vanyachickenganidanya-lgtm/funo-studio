# Funo Studio 1.0 — язык, CLI и Minecraft SDK

**Funo** — простой язык программирования, который компилируется в Java/JVM. В репозитории находятся красивый desktop-редактор на Tauri 2, настоящий консольный компилятор `funo`, менеджер библиотек и генератор Minecraft-модов для Fabric, Forge и NeoForge.

Официальный реестр библиотек:

**https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL**

> Сейчас в официальном репозитории есть только `README.md`. Чтобы команды `funo pkg list/install` увидели пакеты, опубликуйте там `index.json` и папку `packages` из `registry-template/`.

## Что умеет Funo

- типы `byte`, `short`, `int`, `long`, `float`, `double`, `number`, `text`, `bool`, `char`, `any`;
- массивы `int[]` и коллекции `list<T>`, `set<T>`, `map<K,V>`;
- переменные `let`, `var`, `const` и привычная запись `int score = 10`;
- функции с выводимыми или явными типами;
- Python-style f-строки `f"Игрок {player}: {score}"` во всех backend: Java/JVM, Minecraft, C++ 17, Rust, JavaScript и Python;
- `if/else`, короткий `if … then … else`, `while`, `for … in`, `repeat`, `break`, `continue`;
- ввод `readln`, `readInt`, `readLong`, `readDouble`, `readBool`;
- компиляция в Java, `.class` и запускаемый `.jar`;
- автоматическое подключение установленных Funo- и Java-библиотек;
- события и команды Minecraft без написания Java-моста вручную;
- Monaco Editor с подсветкой, подсказками, hover-документацией и дружелюбными ошибками;
- режимы интерфейса «Новичок» и «Профи».

## Пример обычной программы

```funo
fun fib(n: int) -> int = if n < 2 then n else fib(n - 1) + fib(n - 2)

fun main() {
    text player = "Alex"
    int score = fib(10)
    bool winner = score >= 50
    int[] rewards = [3, 5, 8]
    list<text> worlds = ["overworld", "nether"]

    if winner {
        println(f"{player} победил: {score}")
    } else {
        println("Попробуй ещё раз")
    }

    for i in 0..3 {
        println(rewards[i])
    }

    return(200)
}
```

Точки с запятой не обязательны. `return(200)` в `main` означает успешное завершение Funo и преобразуется в обычный `return` JVM.

### F-строки во всех backend

```funo
text name = "Alex"
int score = 42
println(f"Игрок {name}: {score}")
println(f"Фигурные скобки: {{готово}}")
```

Внутри `{…}` можно использовать выражение. `{{` и `}}` выводят обычные фигурные скобки. Компилятор опускает этот синтаксис в Java-конкатенацию, `funo_concat` для C++, `format!` для Rust, template literals для JavaScript и нативные f-строки Python. В Minecraft выражение `{player}` выводит имя игрока, а не внутренний объект загрузчика.

### Коллекции

```funo
fun main() {
    list<text> players = ["Alex", "Steve"]
    set<int> ids = [10, 20, 20]
    map<text, int> scores = map()

    players.add("Sunny")
    scores.put("Alex", 42)
    println(len(players))
    println(scores.get("Alex"))
}
```

Коллекции изменяемые и используют знакомые JVM-методы: `add`, `remove`, `contains`, `get`, `put`, `clear`. Функции `list(…)`, `set(…)` и `map()` позволяют создавать их выражением.

### Переменные и ввод

```funo
fun main() {
    print("Ваш возраст: ")
    int age = readInt()
    let adult: bool = age >= 18
    var attempts = 3

    while attempts > 0 and not adult {
        println("Осталось попыток: " + attempts)
        attempts = attempts - 1
    }
}
```

## CLI и установка в PATH

### Linux/macOS

```bash
./scripts/install-cli.sh
```

### Windows PowerShell

```powershell
.\scripts\install-cli.ps1
```

Скрипт собирает бинарник, запускает `funo setup`, копирует его в пользовательскую папку и добавляет эту папку в пользовательский `PATH`. Права администратора не нужны. После установки откройте новый терминал.

Можно сделать то же вручную:

```bash
cargo build --release --locked --manifest-path src-tauri/Cargo.toml --bin funo
./src-tauri/target/release/funo setup
```

Основные команды:

```bash
funo check main.fun                 # проверить синтаксис
funo run main.fun                   # собрать и запустить
funo build main.fun                 # .funo/build/app.jar
funo build main.fun -o my-app.jar   # выбрать имя результата
funo java main.fun                  # показать созданный Java-код
funo --help
```

Для `run/build` требуется JDK 17 или 21 (`java`, `javac` и `jar` в PATH). Команда `check` работает без JDK.

## Библиотеки из официального GitHub

```bash
funo pkg list
funo pkg search minecraft
funo pkg install funo.hello
funo pkg install funo.hello@1.0.0
funo pkg remove funo.hello
```

Менеджер пакетов:

1. загружает `index.json` из `funo_libsOFFICAL`;
2. разрешает только HTTPS;
3. ограничивает пакет размером 100 МБ;
4. сверяет SHA-256;
5. безопасно распаковывает `.funpkg` без `../` и абсолютных путей;
6. записывает точную версию и хеш в `funo.lock`;
7. автоматически добавляет установленные `.jar` в classpath компилятора.

Непроверенный пакет блокируется. Флаг `--unsafe` существует для разработки, но использовать его стоит только для доверенного источника.

Формат первого пакета и готовый `index.json` находятся в [`registry-template/`](registry-template/README.md).

## Minecraft-моды на Funo

Создание проекта из Studio — значок куба слева. Из терминала:

```bash
# Каталог меняется вместе с официальными релизами загрузчика
funo minecraft versions fabric
funo minecraft versions forge
funo minecraft versions neoforge

# loader и Minecraft version задаются явно
funo minecraft new "Hello Funo" hello_funo fabric 1.21.1
funo minecraft new "Hello Neo" hello_neo neoforge 26.2
cd ~/Documents/FunoProjects/hello_funo
funo minecraft build
```

Поддерживаются `fabric`, `forge` и `neoforge`. Studio и CLI загружают версии из официальных Fabric Meta, Forge Maven и NeoForge Maven, поэтому список не зашит в приложение и получает новые релизы без обновления Funo. Доступны все опубликованные стабильные и preview-версии, для которых современный генератор может создать проект: Fabric от 1.14, Forge от 1.14.4 и NeoForge от 1.20.2, включая календарные версии 26.x.

Генератор разрешает точную совместимую версию Loader, Fabric API и mappings, создаёт Gradle-конфигурацию нужной эпохи, manifest загрузчика, отражательный Java-мост событий и runtime `FunoMinecraft`. Версия Java подбирается автоматически:

| Minecraft | Java |
| --- | ---: |
| до 1.16.x | 8 |
| 1.17.x | 16 |
| 1.18–1.20.4 | 17 |
| 1.20.5–1.21.x | 21 |
| календарные 26.x | 25 |

Если версия в команде `new` не указана, CLI выбирает самый новый доступный стабильный релиз выбранного загрузчика.

```funo
use minecraft.fabric

mod "hello_funo" {
    on start {
        log("Мод загружен")
    }

    on server_start {
        broadcast("Сервер Funo запущен!")
        actionbar("Добро пожаловать")
        run_command("time set day")
    }

    on player_join(player) {
        tell(f"Привет, {player}!")
        // damage(2)
        // tp("~", "~1", "~")
    }

    on block_break(player, block) {
        broadcast(f"{player} сломал {block}")
        give("minecraft:diamond", 1)
    }

    on player_leave(player) {
        log(f"{player} покинул мир")
    }

    // Универсальный fallback для доступных событий конкретного loader/version.
    on player_event(player, event, detail) {
        log(f"{player}: {event} — {detail}")
    }
}
```

События загрузки:

- `on start` — загрузка мода;
- `on server_start` — сервер полностью запущен;
- `on player_join(player)` — игрок вошёл в мир.

Внутримировые события игрока:

| Группа | Обработчики |
| --- | --- |
| Жизненный цикл | `player_leave(player)`, `player_tick(player)`, `player_respawn(player, detail)`, `player_death(player, detail)` |
| Блоки | `block_break(player, block)`, `block_place(player, block)`, `block_interact(player, block)` |
| Предметы | `item_use(player, item)`, `item_pickup(player, item)`, `item_drop(player, item)`, `item_craft(player, item)`, `item_smelt(player, item)` |
| Сущности и бой | `entity_interact(player, entity)`, `entity_attack(player, entity)`, `entity_kill(player, entity)`, `player_damage(player, amount)` |
| Мир и общение | `dimension_change(player, dimension)`, `chat(player, message)`, `command(player, command)`, `container_open(player, container)`, `container_close(player, container)` |
| Остальное в мире | `player_sleep(player, detail)`, `player_wake(player, detail)`, `advancement(player, advancement)`, `player_jump(player, detail)` |
| Version-neutral fallback | `player_event(player, event, detail)` |

`player` всегда является конкретным исполнителем/участником события. Второй аргумент именованного события содержит доступный контекст загрузчика. Forge и NeoForge подписываются на общий event bus и классифицируют player-события отражательно; Fabric отражательно регистрирует callback-и, доступные в выбранной версии Fabric API. Callback-и с возвращаемым значением получают неинвазивный `PASS`/безопасный default. `player_event` позволяет обработать доступное событие, даже если у него нет отдельного стабильного имени Funo.

Это одинаково работает на выделенном сервере и в одиночном мире: одиночная игра запускает встроенный сервер, а локальный пользователь подключается к нему как игрок.

Minecraft API Funo:

- `log(value)` — запись в лог;
- `broadcast(text)` — сообщение всем игрокам;
- `actionbar(text)` — текст над панелью быстрого доступа;
- `tell(text)` — личное сообщение текущему игроку;
- `give("minecraft:item", count)` — выдать предмет;
- `damage(amount)` — нанести урон только текущему игроку (`2` = одно полное сердце до учёта брони и эффектов; требуется Minecraft 1.19.4+);
- `tp(x, y, z)` — телепортировать только текущего игрока, включая относительные координаты строками (`"~"`, `"~1"`);
- `run_command("…")` — выполнить серверную команду.

`tell`, `give`, `give_custom`, `damage` и `tp` доступны внутри любого player-события и автоматически получают конкретного `player`: широкие селекторы вроде `@a` не используются. При каждой сборке Studio обновляет и `FunoMinecraft.java`, и loader-мост `FunoMod.java`, поэтому f-строки, команды и новые события появляются в ранее созданных проектах без пересоздания.

При сборке `main.fun` превращается в `FunoMain.java`, затем запускается Gradle. Итоговый JAR появляется в `build/libs/`.

## Funo Studio

Требования для разработки:

- Node.js 20+;
- Rust stable 1.77.2+;
- системные зависимости Tauri 2;
- JDK 17/21;
- JDK 21 и Gradle для Minecraft.

Запуск:

```bash
npm install
npm run tauri:dev
```

Только web-интерфейс:

```bash
npm run dev
```

Сборка desktop-установщика:

```bash
npm run tauri:build
```

GitHub Actions собирает Windows `.msi`/`.exe` и отдельный `funo.exe`, который можно добавить в PATH командой `funo setup`.

### Linux (Debian/Ubuntu)

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Структура проекта

```text
src/                         TypeScript UI и язык Monaco
src-tauri/src/compiler.rs    Funo → Java, javac/java/jar, Minecraft compiler
src-tauri/src/cli.rs         команда funo
src-tauri/src/registry.rs    официальный менеджер пакетов
src-tauri/src/project.rs     каталоги версий и Fabric/Forge/NeoForge generator
scripts/                     установка CLI в PATH
registry-template/           шаблон официального реестра
examples/                    примеры Funo
```

## Ограничения 0.3

- Funo уже подходит для консольных учебных и небольших JVM-программ, но это ещё не полный аналог Java/Kotlin.
- Диагностика самого Funo дружелюбная; ошибки типов стороннего Java API пока выводит `javac`.
- Minecraft runtime подключает события Fabric/Forge/NeoForge и серверные команды через совместимый между версиями отражательный мост; регистрация собственных блоков, предметов и GUI будет добавляться отдельными адаптерами Funo Pack.
- SHA-256 защищает от незаметной подмены файла, но будущей схеме официального реестра также нужны цифровые подписи.
