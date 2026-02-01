fun fib(n: Int): Long {
    if (n < 2) return n.toLong()
    return fib(n - 1) + fib(n - 2)
}

fun main() {
    val start = System.nanoTime()
    val res = fib(30)
    val end = System.nanoTime()
    
    val durationMs = (end - start) / 1_000_000.0
    
    println("Kotlin (Recursive Fib 30): $res")
    println("Time: %.2fms".format(durationMs))
}
