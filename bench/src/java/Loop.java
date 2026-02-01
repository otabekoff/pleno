public class Loop {
    public static void main(String[] args) {
        long start = System.nanoTime();
        long sum = 0;
        for (int i = 0; i < 10000000; i++) {
            sum += i;
        }
        long end = System.nanoTime();
        
        double durationMs = (end - start) / 1_000_000.0;
        
        System.out.println("Java (Loop Sum 10M): " + sum);
        System.out.printf("Time: %.2fms%n", durationMs);
    }
}
