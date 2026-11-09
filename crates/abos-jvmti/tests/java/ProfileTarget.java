/** A deterministic hot loop for compiled-method and inline-metadata callbacks. */
public final class ProfileTarget {
    private static int inner(int value) {
        return value * 31 + 7;
    }

    private static int outer(int value) {
        return inner(value) ^ inner(value + 1);
    }

    public static void main(String[] args) {
        int sum = 0;
        for (int i = 0; i < 1000000; i++) {
            sum += outer(i);
        }
        System.out.println("PROFILE_TARGET_OK " + sum);
    }
}
