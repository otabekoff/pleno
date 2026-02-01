import java.util.stream.IntStream;

public class Array {
    public static void main(String[] args) {
        long start = System.nanoTime();

        // Stream: range -> map -> filter -> sum
        // Note: IntStream is efficient primitive stream, avoids boxing overhead of Stream<Integer>
        long sum = IntStream.range(0, 1000000)
                .map(x -> x * 2)
                .filter(x -> x % 3 == 0)
                .asLongStream()
                .sum();

        long end = System.nanoTime();
        
        double durationMs = (end - start) / 1_000_000.0;
        
        System.out.println("Java (Array Ops 1M): " + sum);
        System.out.printf("Time: %.2fms%n", durationMs);
    }
}
