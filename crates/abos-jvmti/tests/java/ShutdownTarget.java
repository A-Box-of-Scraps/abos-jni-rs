public final class ShutdownTarget {
    private static native int pollShutdown();

    private static native long callbackCount();

    private static void worker() throws Exception {
        Thread thread = new Thread(() -> {}, "shutdown-worker");
        thread.start();
        thread.join();
    }

    public static void main(String[] args) throws Exception {
        worker();
        if (callbackCount() == 0) {
            throw new AssertionError("no ThreadStart callback before shutdown");
        }
        long deadline = System.nanoTime() + 5000000000L;
        int status;
        while ((status = pollShutdown()) == 0) {
            if (System.nanoTime() > deadline) {
                throw new AssertionError("shutdown did not drain");
            }
            Thread.sleep(1);
        }
        if (status != 1 || pollShutdown() != 1) {
            throw new AssertionError("shutdown failed or was not idempotent");
        }
        long stopped = callbackCount();
        worker();
        worker();
        if (callbackCount() != stopped) {
            throw new AssertionError("callback accessed agent resources after shutdown");
        }
        System.out.println("SHUTDOWN_TARGET_OK");
    }
}
