import * as monaco from 'monaco-editor';
import type { Diagnostic } from './api';

export function registerFunoLanguage() {
  monaco.languages.register({ id: 'funo', extensions: ['.fun'], aliases: ['Funo', 'funo'] });

  monaco.languages.setLanguageConfiguration('funo', {
    comments: { lineComment: '//', blockComment: ['/*', '*/'] },
    brackets: [['{', '}'], ['[', ']'], ['(', ')']],
    autoClosingPairs: [
      { open: '{', close: '}' }, { open: '[', close: ']' },
      { open: '(', close: ')' }, { open: '"', close: '"' }, { open: "'", close: "'" }
    ],
    surroundingPairs: [
      { open: '{', close: '}' }, { open: '[', close: ']' },
      { open: '(', close: ')' }, { open: '"', close: '"' }
    ],
    indentationRules: {
      increaseIndentPattern: /\{[^}"']*$/,
      decreaseIndentPattern: /^\s*\}/
    }
  });

  monaco.languages.setMonarchTokensProvider('funo', {
    defaultToken: '',
    tokenPostfix: '.funo',
    keywords: ['fun', 'let', 'var', 'const', 'if', 'then', 'else', 'return', 'use', 'java', 'lib', 'public', 'mod', 'on', 'start', 'server_start', 'player_join', 'for', 'in', 'while', 'repeat', 'break', 'continue', 'new', 'true', 'false', 'null', 'and', 'or', 'not'],
    typeKeywords: ['byte', 'short', 'int', 'long', 'float', 'double', 'number', 'decimal', 'text', 'string', 'bool', 'boolean', 'char', 'list', 'set', 'map', 'any', 'void'],
    builtins: ['println', 'print', 'readln', 'readInt', 'readLong', 'readDouble', 'readBool', 'toInt', 'toDouble', 'len', 'list', 'set', 'map', 'minecraft', 'fabric', 'forge', 'neoforge', 'log', 'broadcast', 'tell', 'give', 'give_custom', 'define_item', 'craft_shapeless', 'craft_shaped', 'spawn_mob', 'set_mob_ai', 'mob_attribute', 'run_command', 'actionbar'],
    operators: ['=', '>', '<', '!', '~', '?', ':', '==', '<=', '>=', '!=', '&&', '||', '+', '-', '*', '/', '^', '%', '->'],
    symbols: /[=><!~?:&|+\-*\/\^%]+/,
    tokenizer: {
      root: [
        [/[a-zA-Z_А-Яа-яЁё][\wА-Яа-яЁё]*/, {
          cases: {
            '@keywords': 'keyword', '@typeKeywords': 'type.identifier',
            '@builtins': 'predefined', '@default': 'identifier'
          }
        }],
        { include: '@whitespace' },
        [/[{}()\[\]]/, '@brackets'],
        [/@symbols/, { cases: { '@operators': 'operator', '@default': '' } }],
        [/\d*\.\d+([eE][\-+]?\d+)?/, 'number.float'],
        [/\d+/, 'number'],
        [/[;,.]/, 'delimiter'],
        [/"([^"\\]|\\.)*$/, 'string.invalid'],
        [/"/, 'string', '@string_double'],
        [/'[^\\']'/, 'string'],
        [/'/, 'string.invalid']
      ],
      whitespace: [[/[ \t\r\n]+/, ''], [/\/\*/, 'comment', '@comment'], [/\/\/.*$/, 'comment']],
      comment: [[/[^\/*]+/, 'comment'], [/\*\//, 'comment', '@pop'], [/[\/*]/, 'comment']],
      string_double: [[/[^\\"]+/, 'string'], [/\\./, 'string.escape.invalid'], [/"/, 'string', '@pop']]
    }
  });

  monaco.languages.registerCompletionItemProvider('funo', {
    triggerCharacters: ['.', ' '],
    provideCompletionItems(model, position) {
      const range = new monaco.Range(position.lineNumber, model.getWordUntilPosition(position).startColumn, position.lineNumber, model.getWordUntilPosition(position).endColumn);
      const item = (label: string, detail: string, insertText: string, kind = monaco.languages.CompletionItemKind.Snippet) => ({
        label, detail, kind, insertText, insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet, range
      });
      return { suggestions: [
        item('fun', 'Новая функция Funo', 'fun ${1:name}(${2}) {\n    ${3}\n}'),
        item('main', 'Точка входа программы', 'fun main() {\n    ${1:println("Привет!")}\n}'),
        item('println', 'Вывести строку или число', 'println(${1:"Привет!"})', monaco.languages.CompletionItemKind.Function),
        item('int variable', 'Целое число', 'int ${1:score} = ${2:0}', monaco.languages.CompletionItemKind.Variable),
        item('text variable', 'Текстовая переменная', 'text ${1:name} = "${2:Steve}"', monaco.languages.CompletionItemKind.Variable),
        item('list<int>', 'Изменяемый список чисел', 'list<int> ${1:values} = [${2:1, 2, 3}]', monaco.languages.CompletionItemKind.Variable),
        item('map<text, int>', 'Изменяемый словарь', 'map<text, int> ${1:scores} = map()\n${1:scores}.put("${2:Alex}", ${3:0})', monaco.languages.CompletionItemKind.Variable),
        item('readInt', 'Прочитать целое число из консоли', 'readInt()', monaco.languages.CompletionItemKind.Function),
        item('if', 'Простое условие', 'if ${1:условие} {\n    ${2}\n} else {\n    ${3}\n}'),
        item('if…then…else', 'Короткое условие-выражение', 'if ${1:условие} then ${2:да} else ${3:нет}'),
        item('for range', 'Цикл по диапазону', 'for ${1:i} in ${2:0}..${3:10} {\n    ${4}\n}'),
        item('while', 'Цикл с условием', 'while ${1:условие} {\n    ${2}\n}'),
        item('minecraft.fabric', 'Подключить Fabric API', 'use minecraft.fabric'),
        item('minecraft.forge', 'Подключить Forge API', 'use minecraft.forge'),
        item('minecraft.neoforge', 'Подключить NeoForge API', 'use minecraft.neoforge'),
        item('minecraft events', 'События сервера и игрока', 'on server_start {\n    broadcast("${1:Сервер запущен!}")\n}\n\non player_join(player) {\n    tell("${2:Добро пожаловать!}")\n}'),
        item('define_item', 'Псевдоним предмета Minecraft', 'define_item("${1:ruby}", "${2:minecraft:emerald}")', monaco.languages.CompletionItemKind.Function),
        item('give_custom', 'Выдать игроку предмет по псевдониму', 'give_custom("${1:ruby}", ${2:1})', monaco.languages.CompletionItemKind.Function),
        item('craft_shapeless', 'Бесформенный рецепт с учётом версии Minecraft', 'craft_shapeless("${1:ruby_block}", "${2:minecraft:emerald_block}", ${3:1}, "${4:minecraft:emerald}", "${5:minecraft:emerald}")', monaco.languages.CompletionItemKind.Function),
        item('craft_shaped', 'Форменный рецепт: три строки и ключи', 'craft_shaped("${1:ruby_pickaxe}", "${2:minecraft:diamond_pickaxe}", ${3:1}, "${4:RRR}", "${5: S }", "${6: S }", "R=minecraft:emerald", "S=minecraft:stick")', monaco.languages.CompletionItemKind.Function),
        item('spawn_mob', 'Создать моба в мире', 'spawn_mob("${1:minecraft:zombie}", ${2:0}, ${3:80}, ${4:0})', monaco.languages.CompletionItemKind.Function),
        item('set_mob_ai', 'Управлять AI ближайших мобов этого типа', 'set_mob_ai("${1:minecraft:zombie}", ${2:true})', monaco.languages.CompletionItemKind.Function),
        item('mob_attribute', 'Изменить атрибут ближайшего моба', 'mob_attribute("${1:minecraft:zombie}", "${2:minecraft:generic.max_health}", ${3:40.0})', monaco.languages.CompletionItemKind.Function),
        item('native-ready main', 'Программа для JVM, C++ 17, Rust, JS и Python', 'fun main() {\n    int ${1:value} = ${2:42}\n    for i in 0..${3:5} {\n        println(${1:value} + i)\n    }\n}'),
        item('java import', 'Подключить класс Java', 'use java "${1:java.util.ArrayList}"'),
        item('return(200)', 'Явное успешное завершение Funo', 'return(200)')
      ] };
    }
  });

  monaco.languages.registerHoverProvider('funo', {
    provideHover(model, position) {
      const word = model.getWordAtPosition(position)?.word;
      const docs: Record<string, string> = {
        fun: '**fun** создаёт функцию. Для короткой функции можно написать `fun double(n) = n * 2`.',
        println: '**println(value)** выводит значение и переходит на новую строку.',
        int: '**int** — 32-битное целое число. Пример: `int score = 10`.',
        text: '**text** — строка Unicode. Пример: `text name = "Alex"`.',
        list: '**list<T>** — изменяемый список. Пример: `list<int> ids = [1, 2, 3]`.',
        set: '**set<T>** — изменяемое множество уникальных значений.',
        map: '**map<K, V>** — изменяемый словарь. Пример: `map<text, int> scores = map()`.',
        readInt: '**readInt()** читает строку из консоли и преобразует её в `int`.',
        return: '**return(value)** возвращает значение. В `main` запись `return(200)` означает успех Funo и необязательна.',
        use: '**use** подключает пакет Funo или класс Java.',
        mod: '**mod** описывает Minecraft-мод простым блоком.',
        broadcast: '**broadcast(text)** отправляет сообщение всем игрокам после события `server_start`.',
        give: '**give(item, count)** выдаёт предмет игроку внутри `on player_join(player)`.',
        define_item: '**define_item(alias, itemId)** создаёт удобный псевдоним существующего Minecraft-предмета. Например, `define_item("ruby", "minecraft:emerald")`.',
        give_custom: '**give_custom(alias, count)** выдаёт предмет, ранее объявленный через `define_item`.',
        craft_shapeless: '**craft_shapeless(id, result, count, ...ingredients)** создаёт JSON-рецепт. Funo сам выбирает схему для версии Minecraft.',
        craft_shaped: '**craft_shaped(id, result, count, row1, row2, row3, ...keys)** создаёт форменный рецепт. Ключ выглядит как `"R=minecraft:redstone"`.',
        spawn_mob: '**spawn_mob(entityId, x, y, z)** создаёт моба через серверный API.',
        set_mob_ai: '**set_mob_ai(entityId, enabled)** включает или отключает AI ближайших мобов выбранного типа.',
        mob_attribute: '**mob_attribute(entityId, attributeId, value)** меняет базовый атрибут ближайшего моба.'
      };
      return word && docs[word] ? { range: new monaco.Range(position.lineNumber, position.column, position.lineNumber, position.column + word.length), contents: [{ value: docs[word] }] } : null;
    }
  });
}

export function setDiagnostics(model: monaco.editor.ITextModel, diagnostics: Diagnostic[]) {
  const severity = (s: Diagnostic['severity']) => s === 'error' ? monaco.MarkerSeverity.Error : s === 'warning' ? monaco.MarkerSeverity.Warning : monaco.MarkerSeverity.Info;
  monaco.editor.setModelMarkers(model, 'funo', diagnostics.map(d => ({
    startLineNumber: d.line,
    startColumn: d.column,
    endLineNumber: d.line,
    endColumn: Math.max(d.end_column, d.column + 1),
    severity: severity(d.severity),
    code: d.code,
    message: `${d.title}\n${d.message}${d.example ? `\n\nПример:\n${d.example}` : ''}`
  })));
}
