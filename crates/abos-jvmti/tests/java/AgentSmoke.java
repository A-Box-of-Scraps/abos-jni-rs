public final class AgentSmoke {
    public static void main(String[] args) throws Exception {
        Thread worker = new Thread(() -> System.out.println("JVMTI_WORKER_OK"), "jvmti-smoke-worker");
        worker.start();
        worker.join();
        if (args.length > 0) {
            AgentFeatures.run();
        }
    }
}
