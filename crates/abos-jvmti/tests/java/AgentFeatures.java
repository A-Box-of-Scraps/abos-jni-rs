import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.locks.LockSupport;

public final class AgentFeatures {
    private static volatile boolean running;

    private static native void configure(Class<?> target, byte[] bytes);

    private static native void exercise(Thread worker, Object root, boolean virtual);

    private static native void completed();

    private static void testThread(boolean virtual) throws Exception {
        CountDownLatch ready = new CountDownLatch(1);
        running = true;
        Runnable task = () -> {
            ready.countDown();
            while (running) {
                LockSupport.parkNanos(1000000);
            }
        };
        Thread worker;
        if (virtual) {
            worker = (Thread)
                    Thread.class.getMethod("startVirtualThread", Runnable.class).invoke(null, task);
        } else {
            worker = new Thread(task, "jvmti-feature-worker");
            worker.start();
        }
        ready.await();
        try {
            Thread.currentThread().interrupt();
            exercise(worker, new Object[] {new Object()}, virtual);
        } finally {
            Thread.interrupted();
            running = false;
            LockSupport.unpark(worker);
            worker.join();
        }
    }

    public static void run() throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (InputStream input = FeatureTarget.class.getResourceAsStream("/FeatureTarget.class")) {
            byte[] buffer = new byte[4096];
            int count;
            while ((count = input.read(buffer)) != -1) {
                bytes.write(buffer, 0, count);
            }
        }
        configure(FeatureTarget.class, bytes.toByteArray());
        long result = new FeatureTarget().compute(7, 8L, 3.5f, 4.5, new Object());
        if (result != 129) {
            throw new AssertionError("JVMTI local changes not observed: " + result);
        }
        testThread(false);
        try {
            Thread.class.getMethod("startVirtualThread", Runnable.class);
            testThread(true);
        } catch (NoSuchMethodException baseline) {
            System.out.println("JVMTI_VIRTUAL_THREADS_UNAVAILABLE");
        }
        completed();
    }
}
