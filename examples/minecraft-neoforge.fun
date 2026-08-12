use minecraft.neoforge

mod "hello_funo_neoforge" {
    on start {
        log("NeoForge-мод Funo загружен")
    }

    on server_start {
        broadcast("NeoForge-сервер с Funo запущен!")
        actionbar("Добро пожаловать")
    }

    on player_join(player) {
        tell("Привет из Funo + NeoForge!")
        give("minecraft:emerald", 1)
        // damage(2) наносит одно сердце урона только этому игроку
        // tp("~", "~1", "~") телепортирует только этого игрока
    }
}
