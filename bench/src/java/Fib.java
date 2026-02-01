public class Fib {
    public static long fib(int n) {
        if (n < 2) return n;
        return fib(n - 1) + fib(n - 2);
    }

    public static void main(String[] args) {
        long start = System.nanoTime();
        long res = fib(30);
        long end = System.nanoTime();
        
        double durationMs = (end - start) / 1_000_000.0;
        
        System.out.println("Java (Recursive Fib 30): " + res);
        System.out.printf("Time: %.2fms%n", durationMs);
    }
}
