package io.github.jni_rs.jbindgen.testactivity;

import java.util.ArrayList;
import java.util.List;

final class TestSession {
    interface NativeApi {
        void nativeOnCreate();

        String nativeGetMessage();

        int nativeProcessData(int value);

        int nativeGetTestCount();

        String nativeGetTestName(int index);

        String nativeRunTest(int index);

        String callbackMessage();
    }

    private interface Check {
        String run();
    }

    private static final class Case {
        final String name;
        final Check check;

        Case(String name, Check check) {
            this.name = name;
            this.check = check;
        }
    }

    private final List<Case> cases = new ArrayList<>();
    private int next;
    private int passed;
    private int failed;

    TestSession(NativeApi api) {
        cases.add(new Case("jni_callback", () -> {
            api.nativeOnCreate();
            require("Hello from Rust native onCreate!".equals(api.callbackMessage()), "Rust-to-Java callback mismatch");
            return "Rust-to-Java callback passed";
        }));
        cases.add(new Case("jni_string", () -> {
            require("Message from Rust!".equals(api.nativeGetMessage()), "native message mismatch");
            return "Java-to-Rust string result passed";
        }));
        cases.add(new Case("jni_integer", () -> {
            require(api.nativeProcessData(42) == 84, "native integer result mismatch");
            return "Java-to-Rust integer result passed";
        }));
        int count = api.nativeGetTestCount();
        require(count >= 0 && count <= 6, "invalid SDK test count: " + count);
        for (int index = 0; index < count; index++) {
            final int testIndex = index;
            String name = api.nativeGetTestName(index);
            require(name != null && !name.isEmpty(), "empty SDK test name");
            require(cases.stream().noneMatch(test -> test.name.equals(name)), "duplicate test name: " + name);
            cases.add(new Case(name, () -> api.nativeRunTest(testIndex)));
        }
    }

    TestResult runNext() {
        if (complete()) {
            throw new IllegalStateException("test session is complete");
        }
        Case test = cases.get(next++);
        try {
            String message = test.check.run();
            require(message != null, "test returned no result");
            passed++;
            return new TestResult(test.name, TestResult.Status.PASSED, message);
        } catch (RuntimeException | LinkageError | AssertionError error) {
            failed++;
            return new TestResult(test.name, TestResult.Status.FAILED, error.toString());
        }
    }

    int selected() {
        return cases.size();
    }

    int executed() {
        return next;
    }

    int passed() {
        return passed;
    }

    int failed() {
        return failed;
    }

    boolean complete() {
        return next == cases.size();
    }

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }
}
