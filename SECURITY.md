# Безопасность · Security

Path-of-craft-tree — нативный мод Horizon. Его код компилируется прямо в игру и в сервер и работает с правами пользователя, без песочницы. Поэтому лаунчер просит подтвердить доверие к моду (`--trust-native-code`). Перед установкой прочитайте код или ставьте мод только из этого репозитория по тегу релиза.

Что мод делает за пределами игры:

- пишет файлы только в папку мира (`<мир>/path_of_craft_tree/`) и в `config/path_of_craft_tree-server.json` сервера;
- не открывает сетевых соединений: клиент и сервер обмениваются сообщениями через соединение самой игры;
- журнал темпа вместо игрока хранит солёный хеш его UUID; в файл таблицы лидеров попадают имя и UUID только тех игроков, которые сами включили `/pct leaderboard on`.

## Как сообщить об уязвимости

Не открывайте публичный issue. Используйте [приватное сообщение об уязвимости](https://github.com/Betreazen/path-of-craft-tree/security/advisories/new) на GitHub. Ответ — в течение нескольких дней. Исправление выйдет в ближайшем релизе с благодарностью в списке изменений, если вы не против.

---

Path-of-craft-tree is a native Horizon mod: it is compiled into the game and the server and runs with your rights, unsandboxed. It writes only inside the world folder and the server's config file and opens no network connections of its own. Report vulnerabilities privately through [GitHub security advisories](https://github.com/Betreazen/path-of-craft-tree/security/advisories/new), not in public issues.
