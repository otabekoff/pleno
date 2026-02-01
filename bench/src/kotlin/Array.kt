fun main() {
    val start = System.nanoTime()

    // Create range, then map, filter, sum
    // In Kotlin, Sequence suggests lazy, Iterable suggests eager.
    // 'map' on IntRange creates ArrayList.
    val sum = (0 until 1000000)
        .map { it * 2 }
        .filter { it % 3 == 0 }
        .sumOf { it.toLong() }

    val end = System.nanoTime()
    val durationMs = (end - start) / 1_000_000.0
    
    println("Kotlin (Array Ops 1M): $sum")
    println("Time: %.2fms".format(durationMs))
}
