use minecraft.forge

mod "hello_funo_forge" {
    on start {
        log("Forge-мод Funo загружен")
    }

    on server_start {
        broadcast("Forge-сервер с Funo запущен!")
    }

    on player_join(player) {
        tell(f"Привет из Funo + Forge, {player}!")
    }

    on block_place(player, block) {
        log(f"{player} поставил {block}")
    }

    on entity_attack(player, entity) {
        actionbar(f"Цель: {entity}")
    }

    on chat(player, message) {
        log(f"Чат {player}: {message}")
    }
}
