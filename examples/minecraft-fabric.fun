use minecraft.fabric

mod "hello_funo" {
    on start {
        log("Мод Hello Funo загружен")
    }

    on server_start {
        broadcast("Сервер Funo запущен!")
        run_command("time set day")
    }

    on player_join(player) {
        tell(f"Привет, {player}!")
        give("minecraft:diamond", 1)
    }

    on block_break(player, block) {
        broadcast(f"{player} добыл {block}")
    }

    on player_leave(player) {
        log(f"{player} покинул мир")
    }
}
