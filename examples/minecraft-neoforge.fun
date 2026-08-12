use minecraft.neoforge

mod "hello_funo_neoforge" {
    on start {
        log("NeoForge-мод Funo загружен")
    }

    on server_start {
        broadcast("NeoForge-сервер с Funo запущен!")
    }

    on player_join(player) {
        tell(f"Привет из Funo + NeoForge, {player}!")
        give("minecraft:emerald", 1)
        // damage(2) наносит одно сердце урона только этому игроку
        // tp("~", "~1", "~") телепортирует только этого игрока
    }

    on player_damage(player, amount) {
        actionbar(f"{player} получил урон: {amount}")
    }

    on dimension_change(player, dimension) {
        log(f"{player} перешёл в {dimension}")
    }

    on player_event(player, event, detail) {
        log(f"{player}: {event} — {detail}")
    }
}
