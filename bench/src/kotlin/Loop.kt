fun main() {
    val start = System.nanoTime()
    var sum: Long = 0
    for (i in 0 until 10000000) {
        sum += i
    }
    val end = System.nanoTime()
    
    val durationMs = (end - start) / 1_000_000.0
    
    println("Kotlin (Loop Sum 10M): $sum")
    println("Time: %.2fms".format(durationMs))
}
