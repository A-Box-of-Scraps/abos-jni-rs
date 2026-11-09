public final class AgentSmoke {
    public static void main(String[] args) throws InterruptedException {
        Thread worker = new Thread(() -> System.out.println("JVMTI_WORKER_OK"), "jvmti-smoke-worker");
        worker.start();
        worker.join();
    }
}
