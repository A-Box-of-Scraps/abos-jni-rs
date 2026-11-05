package sample;

@Deprecated
public class Legacy<T> implements Runnable {
    public static final long LONG = 1234567890123L;
    public static final double DOUBLE = 1.25;
    public static final String TEXT = "\u0000\ud800";
    public int 𐐀;
    public native String[][] call(int[] values);
    public void run() {
        Runnable task = () -> {};
        task.run();
    }
    public class Inner {}
}
