fun fib(n: int) -> int = if n < 2 then n else fib(n - 1) + fib(n - 2)

fun main() {
    int answer = fib(10)
    println("fib(10) = " + answer)
}
