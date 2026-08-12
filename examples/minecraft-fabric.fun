use minecraft.fabric

mod "hello_funo" {
    on start {
        log("Мод Hello Funo загружен")
    }

    on server_start {
        broadcast("Сервер Funo запущен!")
        actionbar("Добро пожаловать")
        run_command("time set day")
    }

    on player_join(player) {
        tell("Привет из Funo!")
        give("minecraft:diamond", 1)
    }
}
