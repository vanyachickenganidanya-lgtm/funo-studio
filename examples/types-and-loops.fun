fun average(total: double, count: int) -> double = total / count

fun main() {
    text player = "Alex"
    int score = 10
    long seed = 123456789L
    float speed = 1.5f
    double health = 19.75
    bool online = true
    char rank = 'A'
    int[] rewards = [3, 5, 8]
    list<text> worlds = ["overworld", "nether"]

    println(player + " · " + rank)
    for i in 0..3 {
        score = score + rewards[i]
    }

    while health > 0 and online {
        health = health - 5.0
        if health <= 0 {
            break
        }
    }

    println(average(score, 2))
    println(worlds)
}
