# Funo 0.3 — Studio, CLI и Minecraft SDK

**Funo** — простой язык программирования, который компилируется в Java/JVM. В репозитории находятся красивый desktop-редактор на Tauri 2, настоящий консольный компилятор `funo`, менеджер библиотек и генератор Minecraft-модов для Fabric/Forge.

Официальный реестр библиотек:

**https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL**

> Сейчас в официальном репозитории есть только `README.md`. Чтобы команды `funo pkg list/install` увидели пакеты, опубликуйте там `index.json` и папку `packages` из `registry-template/`.

## Что умеет Funo

- типы `byte`, `short`, `int`, `long`, `float`, `double`, `number`, `text`, `bool`, `char`, `any`;
- массивы `int[]` и коллекции `list<T>`, `set<T>`, `map<K,V>`;
- переменные `let`, `var`, `const` и привычная запись `int score = 10`;
- функции с выводимыми или явными типами;
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
        println(player + " победил: " + score)
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
funo minecraft new "Hello Funo" hello_funo fabric
cd ~/Documents/FunoProjects/hello_funo
funo minecraft build
```

Поддерживаются `fabric` и `forge`, Minecraft 1.21.1 и Java 21. Генератор создаёт Gradle-проект, manifest загрузчика, Java-мост событий и runtime `FunoMinecraft`.

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
        tell("Привет!")
        give("minecraft:diamond", 1)
    }
}
```

События:

- `on start` — загрузка мода;
- `on server_start` — сервер полностью запущен;
- `on player_join(player)` — игрок вошёл.

Minecraft API Funo:

- `log(value)` — запись в лог;
- `broadcast(text)` — сообщение всем игрокам;
- `actionbar(text)` — текст над панелью быстрого доступа;
- `tell(text)` — личное сообщение текущему игроку;
- `give("minecraft:item", count)` — выдать предмет;
- `run_command("…")` — выполнить серверную команду.

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
src-tauri/src/project.rs     проекты и Fabric/Forge generator
scripts/                     установка CLI в PATH
registry-template/           шаблон официального реестра
examples/                    примеры Funo
```

## Ограничения 0.3

- Funo уже подходит для консольных учебных и небольших JVM-программ, но это ещё не полный аналог Java/Kotlin.
- Диагностика самого Funo дружелюбная; ошибки типов стороннего Java API пока выводит `javac`.
- Minecraft runtime использует стабильные точки Fabric/Forge и серверные команды; регистрация собственных блоков, предметов и GUI будет добавляться отдельными адаптерами Funo Pack.
- SHA-256 защищает от незаметной подмены файла, но будущей схеме официального реестра также нужны цифровые подписи.
