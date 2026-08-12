use minecraft.forge

mod "hello_funo_forge" {
    on start {
        log("Forge-мод Funo загружен")
    }

    on server_start {
        broadcast("Forge-сервер с Funo запущен!")
        run_command("time set day")
    }

    on player_join(player) {
        tell("Привет из Funo + Forge!")
        give("minecraft:diamond", 1)
    }
}
