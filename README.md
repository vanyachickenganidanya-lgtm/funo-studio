# Funo Studio Desktop (Tauri 2)

Настольный редактор языка **Funo** на Tauri 2: интерфейс TypeScript + Monaco Editor, нативный backend на Rust и сборка программ через установленный JDK.

Официальный реестр подключён к:

**https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL**

На момент создания проекта в репозитории есть только `README.md`, поэтому редактор дружелюбно сообщает, что ожидает `index.json`. Готовые файлы для первой публикации находятся в `registry-template/`.

## Что реализовано по-настоящему

- Tauri 2 desktop shell, а не обёртка только для браузера;
- Monaco Editor — редакторная основа VS Code;
- собственная подсветка, автодополнение и hover-документация Funo;
- автосохранение файлов на диск из Rust;
- нативная дружелюбная диагностика в Rust;
- безопасное автоисправление опечаток и скобок;
- режимы «Новичок» и «Профи»;
- компиляция базового Funo в Java;
- настоящий вызов `javac` и `java` без shell-инъекций;
- `return(200)` в `main` преобразуется в обычное успешное завершение JVM;
- создание Gradle-проектов Minecraft Fabric/Forge;
- генерация Java-моста `FunoMain.java` из блока `on start`;
- запуск `gradle build` для Minecraft-проекта;
- загрузка `index.json` напрямую из вашего GitHub;
- скачивание пакета, ограничение 100 МБ и проверка SHA-256;
- блокировка неподтверждённых пакетов по умолчанию;
- lock-файл `funo.lock`;
- шаблон официального реестра и первого пакета.

## Поддерживаемый синтаксис MVP

```funo
fun fib(n) = if n < 2 then n else fib(n - 1) + fib(n - 2)

fun main() {
    println(fib(10))
    return(200)
}
```

Явные типы остаются необязательными:

```funo
fun double(n: int) -> int = n * 2
```

Java-классы:

```funo
use java "com.google.gson.Gson"
```

Minecraft:

```funo
use minecraft.fabric

mod "hello_funo" {
    on start {
        println("Мод запущен!")
    }
}
```

## Требования

Для разработки:

- Node.js 20+;
- Rust stable 1.77.2+;
- системные зависимости Tauri 2;
- JDK 17 или 21 для обычных программ;
- JDK 21 и Gradle для шаблонов Minecraft 1.21.1.

### Linux (Debian/Ubuntu)

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Запуск в режиме разработки

```bash
npm install
npm run tauri:dev
```

Только интерфейс в браузере:

```bash
npm install
npm run dev
```

В браузере нативные операции заменены демонстрационным адаптером. В окне Tauri используются Rust-команды.

## Сборка установщика

```bash
npm install
npm run tauri:build
```

Результат появится в `src-tauri/target/release/bundle/`.

Для автоматической сборки Windows `.msi`/`.exe` и Linux-пакетов добавлен workflow `.github/workflows/build-desktop.yml`. После загрузки проекта на GitHub запустите его через вкладку **Actions → Build Funo Studio Desktop → Run workflow**.

## Публикация библиотек

Скопируйте содержимое `registry-template/` в корень репозитория `funo_libsOFFICAL`. Пример уже содержит корректную SHA-256 для `hello.funpkg`.

После изменения `.funpkg` обязательно пересчитайте SHA-256:

```bash
sha256sum packages/hello/hello.funpkg
```

## Честные ограничения текущего MVP

- Компилятор пока поддерживает небольшой учебный набор Funo, а не полный Java-язык.
- Minecraft-генератор создаёт реальный Gradle-проект и Java-мост, но сложные события, блоки и предметы потребуют адаптеров из будущих Funo-пакетов.
- Fabric/Forge версии зафиксированы на совместимом шаблоне Minecraft 1.21.1; перед новыми версиями их нужно обновлять и тестировать.
- SHA-256 защищает от незаметного изменения файла, но полноценный официальный реестр должен дополнительно использовать цифровые подписи.
- Непроверенный Java `.jar` нельзя сделать безопасным одной надписью: для него нужен отдельный процесс/песочница и список разрешений.

## Структура

```text
src/                    TypeScript UI и язык Monaco
src-tauri/src/          Rust backend, компилятор и менеджер пакетов
src-tauri/icons/        иконки desktop-приложения
registry-template/      файлы для официального GitHub-реестра
examples/               примеры Funo
```
