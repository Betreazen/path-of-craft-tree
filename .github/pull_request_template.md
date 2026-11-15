## Что изменено

<!-- Кратко: что и зачем. -->

## Проверка

- [ ] `cargo test`
- [ ] `cargo clippy --all-targets -- -D warnings -A dead_code`
- [ ] Если менялись узлы или опыт: `cargo run --example gen_tree` и проверка, что номера старых узлов не сдвинулись (тест `appended_nodes_keep_the_old_numbering`)
- [ ] Если менялись тексты: английский перевод в `mixins/path_of_craft_tree/data/en-*.json`
- [ ] Проверено в игре (что именно):
