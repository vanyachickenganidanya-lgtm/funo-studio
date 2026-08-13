# Funo Studio для Android

Android-версия — это Tauri 2 APK с тем же Monaco-редактором и встроенным Rust-компилятором Funo. Интерфейс адаптирован для телефона: нижняя навигация, выдвижной проводник, крупные touch-элементы, сворачиваемая панель результатов и safe-area отступы.

## Что работает в APK

- редактирование Funo и других файлов проекта;
- локальное автосохранение полного проекта в хранилище приложения;
- создание файлов и папок, поиск и скрытие путей;
- проверка синтаксиса Funo встроенным Rust backend без JDK;
- преобразование обычного и Minecraft Funo в Java для предпросмотра;
- создание исходного проекта Minecraft для Fabric, Forge и NeoForge;
- официальный каталог версий Minecraft и каталог Funo Pack;
- вики, подсказки и интерактивное обучение.

Удаление приложения очищает его локальное хранилище, поэтому важный код следует заранее скопировать в desktop-проект. Обычное обновление APK с той же подписью сохраняет данные.

## Ограничения Android

Android не предоставляет Studio настольные JDK, Gradle, PATH и произвольные CLI-компиляторы. Поэтому в APK намеренно недоступны:

- запуск JVM-программы и создание исполняемого JAR;
- Gradle-сборка JAR Minecraft-мода и Minecraft Launcher;
- установка JDK/Gradle, Modrinth-модов и JVM `.jar`-пакетов;
- C++, Rust, JavaScript и Python backend, которым нужен внешний процесс;
- Plugin SDK и Microsoft-вход лаунчера.

Эти элементы скрыты в мобильном интерфейсе, а frontend и Rust backend дополнительно отклоняют прямой вызов неподдерживаемой операции. Исходник можно проверить на телефоне, а готовый JAR собрать в desktop-версии или CI.

## Готовый APK из GitHub Actions

Workflow [`.github/workflows/build-android.yml`](../.github/workflows/build-android.yml) собирает universal APK для `aarch64`, `armv7`, `i686` и `x86_64` и публикует artifact **`funo-studio-android-apk`**.

1. Откройте вкладку **Actions** репозитория.
2. Выберите **Build Funo Studio Android APK**.
3. Запустите `workflow_dispatch` или откройте завершившийся запуск ветки/PR.
4. Скачайте artifact `funo-studio-android-apk` и распакуйте ZIP.
5. Разрешите установку из выбранного источника на Android и откройте `.apk`.

CI создаёт debug-signed APK: он устанавливается на устройство без закрытого release-ключа и предназначен для тестирования. Для публикации в магазине соберите release APK/AAB со своим закрытым keystore по официальной инструкции Tauri.

## Локальная сборка

Требования:

- Node.js 20+;
- Rust stable;
- JDK 17;
- Android SDK с platform/build-tools 36;
- Android NDK `26.3.11579264` (r26d);
- переменная `NDK_HOME` или `ANDROID_NDK_HOME` с путём к NDK.

Установите Android targets Rust:

```bash
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android
```

Инициализируйте генерируемый Gradle-проект и соберите устанавливаемый debug APK:

```bash
npm ci
npm run android:init
npm run android:build:debug
```

Результат находится в:

```text
src-tauri/gen/android/app/build/outputs/apk/
```

Release APK без debug-подписи:

```bash
npm run android:build
```

Для запуска на подключённом устройстве или эмуляторе:

```bash
npm run android:dev
```

`src-tauri/gen/` намеренно не хранится в Git: `npm run android:init` воспроизводимо создаёт Android Studio/Gradle-проект из конфигурации [`src-tauri/tauri.android.conf.json`](../src-tauri/tauri.android.conf.json).
