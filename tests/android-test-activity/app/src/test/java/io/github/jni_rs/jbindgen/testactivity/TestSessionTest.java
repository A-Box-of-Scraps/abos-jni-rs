package io.github.jni_rs.jbindgen.testactivity;

final class TestSessionTest {
    private static final class FakeApi implements TestSession.NativeApi {
        String callback;
        boolean wrongCallback;
        boolean wrongMessage;
        boolean wrongInteger;
        boolean failSdk;
        int count = 1;
        int lastIndex = -1;

        public void nativeOnCreate() {
            callback = wrongCallback ? "wrong" : "Hello from Rust native onCreate!";
        }

        public String nativeGetMessage() {
            return wrongMessage ? "wrong" : "Message from Rust!";
        }

        public int nativeProcessData(int value) {
            return wrongInteger ? 0 : value * 2;
        }

        public int nativeGetTestCount() {
            return count;
        }

        public String nativeGetTestName(int index) {
            return "sdk_" + index;
        }

        public String callbackMessage() {
            return callback;
        }

        public String nativeRunTest(int index) {
            lastIndex = index;
            if (failSdk) {
                throw new UnsatisfiedLinkError("missing native binding");
            }
            // These words are data, never the test status.
            return "failed errors: 0";
        }
    }

    public static void main(String[] args) {
        verifiesExplicitStatusAndCompletion();
        verifiesFailuresAndContinuation();
        verifiesIndependentSessions();
        verifiesInvalidRegistry();
    }

    private static void verifiesExplicitStatusAndCompletion() {
        FakeApi api = new FakeApi();
        TestSession session = new TestSession(api);
        while (!session.complete()) {
            check(session.runNext().status == TestResult.Status.PASSED, "successful case failed");
        }
        check(session.selected() == 4 && session.executed() == 4, "wrong case counts");
        check(session.passed() == 4 && session.failed() == 0, "wrong outcome counts");
        check(api.lastIndex == 0, "wrong SDK index");
        try {
            session.runNext();
            throw new AssertionError("completed session ran again");
        } catch (IllegalStateException expected) {
            check(session.executed() == 4, "completion changed counts");
        }
    }

    private static void verifiesFailuresAndContinuation() {
        FakeApi api = new FakeApi();
        api.wrongCallback = true;
        api.wrongMessage = true;
        api.wrongInteger = true;
        api.failSdk = true;
        TestSession session = new TestSession(api);
        while (!session.complete()) {
            check(session.runNext().status == TestResult.Status.FAILED, "failure passed");
        }
        check(session.failed() == 4 && session.passed() == 0, "failures were not counted");
    }

    private static void verifiesIndependentSessions() {
        FakeApi api = new FakeApi();
        TestSession first = new TestSession(api);
        while (!first.complete()) {
            first.runNext();
        }
        TestSession second = new TestSession(api);
        check(second.executed() == 0 && second.passed() == 0 && second.failed() == 0, "state leaked across sessions");
        check(second.runNext().name.equals("jni_callback"), "session did not restart");
        api.count = 0;
        TestSession smoke = new TestSession(api);
        check(smoke.selected() == 3, "smoke suite should have three cases");
    }

    private static void verifiesInvalidRegistry() {
        for (int count : new int[] {-1, 7}) {
            FakeApi api = new FakeApi();
            api.count = count;
            boolean rejected = false;
            try {
                new TestSession(api);
            } catch (AssertionError expected) {
                rejected = true;
            }
            check(rejected, "invalid registry accepted");
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }
}
